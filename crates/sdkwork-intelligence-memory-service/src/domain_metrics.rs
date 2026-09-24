use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

static DOMAIN_METRICS: OnceLock<MemoryDomainMetrics> = OnceLock::new();

/// Cumulative upper bounds (inclusive) in milliseconds for latency histograms.
/// Bucket layout targets the documented p99 budgets: normal reads must stay
/// below 200 ms and normal writes below 500 ms, with headroom for async
/// provider-backed work up to the request deadline.
const LATENCY_BOUNDS_MS: &[u64] = &[1, 5, 10, 25, 50, 100, 200, 500, 1000, 2000, 5000, 10000];

/// Cumulative upper bounds (inclusive) in bytes for export payload sizes,
/// spanning the 4 MiB inline default through the 256 MiB hard cap.
const EXPORT_BYTES_BOUNDS: &[u64] = &[
    4096,
    16384,
    65536,
    262144,
    1048576,
    4194304,
    16777216,
    67108864,
    268435456,
];

/// Fixed-bucket Prometheus histogram with atomic counters and no external
/// dependency. Observations are O(buckets) integer adds; rendering is done on
/// scrape only.
struct Histogram {
    bounds: &'static [u64],
    /// One counter per bound plus the implicit +Inf bucket.
    buckets: Vec<AtomicU64>,
    count: AtomicU64,
    sum: AtomicU64,
}

impl Histogram {
    fn new(bounds: &'static [u64]) -> Self {
        Self {
            bounds,
            buckets: (0..bounds.len() + 1).map(|_| AtomicU64::new(0)).collect(),
            count: AtomicU64::new(0),
            sum: AtomicU64::new(0),
        }
    }

    fn observe(&self, value: u64) {
        let index = self
            .bounds
            .iter()
            .position(|bound| value <= *bound)
            .unwrap_or(self.bounds.len());
        self.buckets[index].fetch_add(1, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(value, Ordering::Relaxed);
    }

    fn render(&self, name: &str, help: &str, labels: &str) -> String {
        let mut rendered = String::new();
        rendered.push_str(&format!("# HELP {name} {help}.\n# TYPE {name} histogram\n"));
        let mut cumulative = 0_u64;
        for (index, bound) in self.bounds.iter().enumerate() {
            cumulative += self.buckets[index].load(Ordering::Relaxed);
            rendered.push_str(&format!(
                "{name}_bucket{{{labels},le=\"{bound}\"}} {cumulative}\n"
            ));
        }
        cumulative += self.buckets[self.bounds.len()].load(Ordering::Relaxed);
        rendered.push_str(&format!("{name}_bucket{{{labels},le=\"+Inf\"}} {cumulative}\n"));
        rendered.push_str(&format!(
            "{name}_sum{{{labels}}} {}\n{name}_count{{{labels}}} {}\n",
            self.sum.load(Ordering::Relaxed),
            self.count.load(Ordering::Relaxed),
        ));
        rendered
    }
}

pub struct MemoryDomainMetrics {
    retrieval_total: AtomicU64,
    authz_denied_total: AtomicU64,
    quota_exceeded_total: AtomicU64,
    outbox_published_total: AtomicU64,
    outbox_publish_failed_total: AtomicU64,
    outbox_delivery_failed_total: AtomicU64,
    outbox_dead_letter_total: AtomicU64,
    serving: AtomicU64,
    retrieval_latency: Histogram,
    outbox_delivery_latency: Histogram,
    export_payload_bytes: Histogram,
}

impl MemoryDomainMetrics {
    fn new() -> Self {
        Self {
            retrieval_total: AtomicU64::new(0),
            authz_denied_total: AtomicU64::new(0),
            quota_exceeded_total: AtomicU64::new(0),
            outbox_published_total: AtomicU64::new(0),
            outbox_publish_failed_total: AtomicU64::new(0),
            outbox_delivery_failed_total: AtomicU64::new(0),
            outbox_dead_letter_total: AtomicU64::new(0),
            serving: AtomicU64::new(1),
            retrieval_latency: Histogram::new(LATENCY_BOUNDS_MS),
            outbox_delivery_latency: Histogram::new(LATENCY_BOUNDS_MS),
            export_payload_bytes: Histogram::new(EXPORT_BYTES_BOUNDS),
        }
    }

    pub fn record_retrieval_completed(&self, latency_ms: i64) {
        self.retrieval_total.fetch_add(1, Ordering::Relaxed);
        let latency_ms = u64::try_from(latency_ms.max(0)).unwrap_or(0);
        self.retrieval_latency.observe(latency_ms);
    }

    pub fn record_outbox_delivery_completed(&self, latency_ms: i64) {
        let latency_ms = u64::try_from(latency_ms.max(0)).unwrap_or(0);
        self.outbox_delivery_latency.observe(latency_ms);
    }

    pub fn record_export_payload_bytes(&self, payload_bytes: usize) {
        let payload_bytes = u64::try_from(payload_bytes).unwrap_or(u64::MAX);
        self.export_payload_bytes.observe(payload_bytes);
    }

    pub fn record_authz_denied(&self) {
        self.authz_denied_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_quota_exceeded(&self) {
        self.quota_exceeded_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_outbox_published(&self) {
        self.outbox_published_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_outbox_publish_failed(&self) {
        self.outbox_publish_failed_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_outbox_delivery_failed(&self) {
        self.outbox_delivery_failed_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_outbox_dead_letter(&self) {
        self.outbox_dead_letter_total
            .fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_serving(&self, serving: bool) {
        self.serving
            .store(if serving { 1 } else { 0 }, Ordering::Relaxed);
    }

    pub fn render_prometheus(
        &self,
        service: &str,
        environment: &str,
        deployment_profile: &str,
        runtime_target: &str,
        runtime_profile: &str,
    ) -> String {
        let labels = format!(
            "service=\"{service}\",environment=\"{environment}\",deployment_profile=\"{deployment_profile}\",runtime_target=\"{runtime_target}\",runtime_profile=\"{runtime_profile}\""
        );
        let mut rendered = String::new();
        rendered.push_str(&format!(
            "# HELP memory_retrieval_completed_total Memory retrieval operations completed.\n\
             # TYPE memory_retrieval_completed_total counter\n\
             memory_retrieval_completed_total{{{labels}}} {}\n\
             # HELP memory_authz_denied_total Memory authorization denials at service layer.\n\
             # TYPE memory_authz_denied_total counter\n\
             memory_authz_denied_total{{{labels}}} {}\n\
             # HELP memory_quota_exceeded_total Memory tenant or space quota rejections.\n\
             # TYPE memory_quota_exceeded_total counter\n\
             memory_quota_exceeded_total{{{labels}}} {}\n\
             # HELP memory_outbox_published_total Memory domain outbox events published.\n\
             # TYPE memory_outbox_published_total counter\n\
             memory_outbox_published_total{{{labels}}} {}\n\
             # HELP memory_outbox_publish_failed_total Memory domain outbox claim failures.\n\
             # TYPE memory_outbox_publish_failed_total counter\n\
             memory_outbox_publish_failed_total{{{labels}}} {}\n\
             # HELP memory_outbox_delivery_failed_total Memory domain outbox delivery or ack failures.\n\
             # TYPE memory_outbox_delivery_failed_total counter\n\
             memory_outbox_delivery_failed_total{{{labels}}} {}\n\
             # HELP memory_outbox_dead_letter_total Memory outbox events moved to dead-letter (max retries exceeded).\n\
             # TYPE memory_outbox_dead_letter_total counter\n\
             memory_outbox_dead_letter_total{{{labels}}} {}\n\
             # HELP memory_health_status Memory service health (1=serving, 0=not serving).\n\
             # TYPE memory_health_status gauge\n\
             memory_health_status{{{labels}}} {}\n",
            self.retrieval_total.load(Ordering::Relaxed),
            self.authz_denied_total.load(Ordering::Relaxed),
            self.quota_exceeded_total.load(Ordering::Relaxed),
            self.outbox_published_total.load(Ordering::Relaxed),
            self.outbox_publish_failed_total.load(Ordering::Relaxed),
            self.outbox_delivery_failed_total.load(Ordering::Relaxed),
            self.outbox_dead_letter_total.load(Ordering::Relaxed),
            self.serving.load(Ordering::Relaxed),
        ));
        rendered.push_str(&self.retrieval_latency.render(
            "memory_retrieval_latency_ms",
            "Memory retrieval end-to-end latency in milliseconds",
            &labels,
        ));
        rendered.push_str(&self.outbox_delivery_latency.render(
            "memory_outbox_delivery_latency_ms",
            "Memory outbox webhook delivery latency in milliseconds",
            &labels,
        ));
        rendered.push_str(&self.export_payload_bytes.render(
            "memory_export_payload_bytes",
            "Memory export payload size in bytes",
            &labels,
        ));
        rendered
    }
}

pub fn memory_domain_metrics() -> &'static MemoryDomainMetrics {
    DOMAIN_METRICS.get_or_init(MemoryDomainMetrics::new)
}

pub fn render_memory_domain_prometheus(
    service: &str,
    environment: &str,
    deployment_profile: &str,
    runtime_target: &str,
    runtime_profile: &str,
) -> String {
    memory_domain_metrics().render_prometheus(
        service,
        environment,
        deployment_profile,
        runtime_target,
        runtime_profile,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_render_includes_quota_counter() {
        memory_domain_metrics().record_quota_exceeded();
        let rendered = render_memory_domain_prometheus(
            "sdkwork-api-memory-standalone-gateway",
            "test",
            "standalone",
            "server",
            "sqlite",
        );
        assert!(rendered.contains("memory_quota_exceeded_total"));
        assert!(rendered.contains("# TYPE memory_quota_exceeded_total counter"));
    }

    #[test]
    fn latency_histogram_renders_cumulative_le_buckets() {
        let histogram = Histogram::new(LATENCY_BOUNDS_MS);
        histogram.observe(3);
        histogram.observe(3);
        histogram.observe(150);
        histogram.observe(60_000);
        let rendered = histogram.render("test_latency_ms", "Test latency", "service=\"t\"");
        assert!(rendered.contains("test_latency_ms_bucket{service=\"t\",le=\"5\"} 2"));
        assert!(rendered.contains("test_latency_ms_bucket{service=\"t\",le=\"200\"} 3"));
        assert!(rendered.contains("test_latency_ms_bucket{service=\"t\",le=\"+Inf\"} 4"));
        assert!(rendered.contains("test_latency_ms_count{service=\"t\"} 4"));
        // 3 + 3 + 150 + 60000
        assert!(rendered.contains("test_latency_ms_sum{service=\"t\"} 60156"));
    }

    #[test]
    fn export_byte_histogram_uses_byte_bounds() {
        let histogram = Histogram::new(EXPORT_BYTES_BOUNDS);
        histogram.observe(4096);
        histogram.observe(268_435_457);
        let rendered = histogram.render("test_bytes", "Test bytes", "service=\"t\"");
        assert!(rendered.contains("test_bytes_bucket{service=\"t\",le=\"4096\"} 1"));
        assert!(rendered.contains("test_bytes_bucket{service=\"t\",le=\"+Inf\"} 2"));
    }
}

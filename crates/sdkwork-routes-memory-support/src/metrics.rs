use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sdkwork_utils_rust::is_blank;
use sdkwork_web_core::{HttpMetricsDimensions, HttpMetricsRegistry};

static MEMORY_HTTP_METRICS: OnceLock<Arc<HttpMetricsRegistry>> = OnceLock::new();

fn metric_environment() -> String {
    std::env::var("SDKWORK_MEMORY_ENVIRONMENT")
        .unwrap_or_else(|_| "development".to_owned())
        .to_ascii_lowercase()
}

fn metric_deployment_profile() -> String {
    std::env::var("SDKWORK_MEMORY_DEPLOYMENT_PROFILE").unwrap_or_else(|_| "standalone".to_owned())
}

fn metric_runtime_target() -> String {
    std::env::var("SDKWORK_MEMORY_RUNTIME_TARGET").unwrap_or_else(|_| "server".to_owned())
}

fn metric_runtime_profile() -> String {
    std::env::var("SDKWORK_MEMORY_RUNTIME_PROFILE")
        .ok()
        .filter(|value| !is_blank(Some(value.as_str())))
        .unwrap_or_else(|| {
            // Infer runtime profile from database engine when explicit override is absent.
            let engine = std::env::var("SDKWORK_DATABASE_ENGINE")
                .unwrap_or_else(|_| "sqlite".to_owned())
                .to_ascii_lowercase();
            match engine.as_str() {
                "postgres" | "postgresql" => "postgresql".to_owned(),
                _ => "sqlite".to_owned(),
            }
        })
}

fn memory_http_metric_dimensions() -> HttpMetricsDimensions {
    HttpMetricsDimensions {
        service: "sdkwork-api-memory-standalone-gateway".to_owned(),
        environment: memory_metric_environment_label(),
        deployment_profile: metric_deployment_profile(),
        runtime_target: metric_runtime_target(),
        runtime_profile: metric_runtime_profile(),
    }
}

/// Shared Prometheus registry for all Memory HTTP surfaces (`OBSERVABILITY_SPEC.md` §3).
pub fn memory_http_metrics() -> Arc<HttpMetricsRegistry> {
    MEMORY_HTTP_METRICS
        .get_or_init(|| HttpMetricsRegistry::with_dimensions(memory_http_metric_dimensions()))
        .clone()
}

pub fn refresh_memory_http_metric_dimensions() {
    memory_http_metrics().set_dimensions(memory_http_metric_dimensions());
}

pub fn memory_metric_environment_label() -> String {
    match metric_environment().as_str() {
        "production" | "prod" => "production".to_owned(),
        "test" | "staging" => metric_environment(),
        _ => "development".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Web runtime domain metrics
//
// The shared `HttpMetricsRegistry` above answers "how many requests, how
// long, which status" for every route. The metrics below are the Memory web
// runtime's own domain signals: how long writes take, and how often the rate
// limiter refuses a caller. The fixed-bucket histogram with atomic counters
// mirrors the service layer's `domain_metrics` pattern — observations are
// integer adds, rendering happens on scrape only.
// ---------------------------------------------------------------------------

/// Cumulative upper bounds (inclusive) in seconds for write-request latency.
/// Mirrors the web framework's own HTTP duration buckets, whose top bound
/// covers this runtime's 30 s request deadline.
const WRITE_REQUEST_DURATION_BOUNDS_SECONDS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0,
];

/// The methods counted as writes. Anything else is a read and never observed
/// by [`MemoryWebMetrics::record_write_request_duration`].
const WRITE_METHODS: [&str; 4] = ["POST", "PUT", "PATCH", "DELETE"];

/// Fixed-bucket Prometheus histogram with atomic counters and no external
/// dependency. Observations are held in microseconds (integer adds) and
/// rendered in seconds.
struct DurationHistogram {
    /// Upper bounds in microseconds, derived from the seconds-declared bounds.
    bounds_micros: Vec<u64>,
    /// One counter per bound plus the implicit +Inf bucket.
    buckets: Vec<AtomicU64>,
    count: AtomicU64,
    sum_micros: AtomicU64,
}

impl DurationHistogram {
    fn new(bounds_seconds: &'static [f64]) -> Self {
        Self {
            bounds_micros: bounds_seconds
                .iter()
                .map(|bound| (*bound * 1_000_000.0) as u64)
                .collect(),
            buckets: (0..bounds_seconds.len() + 1)
                .map(|_| AtomicU64::new(0))
                .collect(),
            count: AtomicU64::new(0),
            sum_micros: AtomicU64::new(0),
        }
    }

    fn observe_micros(&self, micros: u64) {
        let index = self
            .bounds_micros
            .iter()
            .position(|bound| micros <= *bound)
            .unwrap_or(self.bounds_micros.len());
        self.buckets[index].fetch_add(1, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum_micros.fetch_add(micros, Ordering::Relaxed);
    }

    fn render(&self, name: &str, help: &str, labels: &str) -> String {
        let mut rendered = format!("# HELP {name} {help}.\n# TYPE {name} histogram\n");
        rendered.push_str(&self.render_series(name, labels));
        rendered
    }

    /// Series lines only — the Prometheus text format allows exactly one
    /// HELP/TYPE block per metric name, so callers aggregating several label
    /// sets under one name emit the header once and this for the rest.
    fn render_series(&self, name: &str, labels: &str) -> String {
        let mut rendered = String::new();
        let mut cumulative = 0_u64;
        for (index, bound) in self.bounds_micros.iter().enumerate() {
            cumulative += self.buckets[index].load(Ordering::Relaxed);
            let bound_seconds = *bound as f64 / 1_000_000.0;
            rendered.push_str(&format!(
                "{name}_bucket{{{labels},le=\"{bound_seconds}\"}} {cumulative}\n"
            ));
        }
        cumulative += self.buckets[self.bounds_micros.len()].load(Ordering::Relaxed);
        rendered.push_str(&format!("{name}_bucket{{{labels},le=\"+Inf\"}} {cumulative}\n"));
        let sum_seconds = self.sum_micros.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        rendered.push_str(&format!(
            "{name}_sum{{{labels}}} {sum_seconds}\n{name}_count{{{labels}}} {}\n",
            self.count.load(Ordering::Relaxed),
        ));
        rendered
    }
}

/// Memory web runtime domain metrics: write-request latency and rate-limit
/// rejections observed by this crate's audit and security-event emitters.
pub struct MemoryWebMetrics {
    /// One histogram per write method, in [`WRITE_METHODS`] order.
    write_request_duration: [DurationHistogram; WRITE_METHODS.len()],
    rate_limit_rejections_total: AtomicU64,
}

impl MemoryWebMetrics {
    fn new() -> Self {
        Self {
            write_request_duration: std::array::from_fn(|_| {
                DurationHistogram::new(WRITE_REQUEST_DURATION_BOUNDS_SECONDS)
            }),
            rate_limit_rejections_total: AtomicU64::new(0),
        }
    }

    /// Records one request's end-to-end duration. Only POST/PUT/PATCH/DELETE
    /// are observed — reads are covered by the shared request-series metrics.
    pub fn record_write_request_duration(&self, method: &str, duration: Duration) {
        let Some(index) = WRITE_METHODS
            .iter()
            .position(|candidate| *candidate == method)
        else {
            return;
        };
        let micros = u64::try_from(duration.as_micros()).unwrap_or(u64::MAX);
        self.write_request_duration[index].observe_micros(micros);
    }

    pub fn record_rate_limit_rejection(&self) {
        self.rate_limit_rejections_total.fetch_add(1, Ordering::Relaxed);
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
            "# HELP memory_rate_limit_rejections_total Requests rejected by the Memory rate limiter.\n\
             # TYPE memory_rate_limit_rejections_total counter\n\
             memory_rate_limit_rejections_total{{{labels}}} {}\n",
            self.rate_limit_rejections_total.load(Ordering::Relaxed),
        ));
        for (index, method) in WRITE_METHODS.iter().enumerate() {
            let labels = format!("method=\"{method}\",{labels}");
            let series_rendered = if index == 0 {
                self.write_request_duration[index].render(
                    "memory_write_request_duration_seconds",
                    "Memory write request (POST/PUT/PATCH/DELETE) end-to-end duration in seconds",
                    &labels,
                )
            } else {
                self.write_request_duration[index]
                    .render_series("memory_write_request_duration_seconds", &labels)
            };
            rendered.push_str(&series_rendered);
        }
        rendered
    }
}

static MEMORY_WEB_METRICS: OnceLock<MemoryWebMetrics> = OnceLock::new();

/// Process-wide Memory web runtime metrics.
pub fn memory_web_metrics() -> &'static MemoryWebMetrics {
    MEMORY_WEB_METRICS.get_or_init(MemoryWebMetrics::new)
}

/// Renders the Memory web runtime metrics with the shared dimension labels,
/// in the Prometheus text format the `/metrics` endpoint concatenates.
pub fn render_memory_web_prometheus() -> String {
    let dimensions = memory_http_metric_dimensions();
    memory_web_metrics().render_prometheus(
        &dimensions.service,
        &dimensions.environment,
        &dimensions.deployment_profile,
        &dimensions.runtime_target,
        &dimensions.runtime_profile,
    )
}

#[cfg(test)]
mod web_metrics_tests {
    use super::*;

    fn render(metrics: &MemoryWebMetrics) -> String {
        metrics.render_prometheus("svc", "development", "standalone", "server", "sqlite")
    }

    #[test]
    fn write_request_duration_is_observed_per_write_method_in_seconds() {
        let metrics = MemoryWebMetrics::new();
        metrics.record_write_request_duration("POST", Duration::from_millis(120));
        metrics.record_write_request_duration("PUT", Duration::from_millis(2_500));
        // Reads are not writes: a GET must not open a series.
        metrics.record_write_request_duration("GET", Duration::from_millis(120));

        let rendered = render(&metrics);
        assert!(
            rendered.contains("memory_write_request_duration_seconds_count{method=\"POST\",service=\"svc\",environment=\"development\",deployment_profile=\"standalone\",runtime_target=\"server\",runtime_profile=\"sqlite\"} 1"),
            "POST series must observe exactly one request: {rendered}"
        );
        // 120 ms lands in the le="0.25" bucket; 2.5 s in le="2.5".
        assert!(rendered.contains("le=\"0.25\"} 1"), "{rendered}");
        assert!(rendered.contains("le=\"2.5\"} 1"), "{rendered}");
        for method in WRITE_METHODS {
            assert!(
                rendered.contains(&format!(
                    "memory_write_request_duration_seconds_bucket{{method=\"{method}\","
                )),
                "every write method must have a series: {rendered}"
            );
        }
        assert!(
            !rendered.contains("method=\"GET\""),
            "reads must not be observed: {rendered}"
        );
    }

    #[test]
    fn rate_limit_rejections_are_counted() {
        let metrics = MemoryWebMetrics::new();
        metrics.record_rate_limit_rejection();
        metrics.record_rate_limit_rejection();

        let rendered = render(&metrics);
        assert!(
            rendered.contains("memory_rate_limit_rejections_total{service=\"svc\",environment=\"development\",deployment_profile=\"standalone\",runtime_target=\"server\",runtime_profile=\"sqlite\"} 2"),
            "both rejections must be counted: {rendered}"
        );
    }
}

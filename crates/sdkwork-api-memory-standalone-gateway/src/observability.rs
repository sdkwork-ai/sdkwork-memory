//! Tracing bootstrap for `sdkwork-api-memory-standalone-gateway` (`OBSERVABILITY_SPEC.md` §2, §4).

#[cfg(feature = "otel")]
use sdkwork_utils_rust::is_blank;

/// Head-sampling ratio read from `SDKWORK_TRACING_SAMPLE_RATIO`.
///
/// Defaults to `1.0` (sample everything); the value is clamped to `[0.0, 1.0]`
/// and an unparseable value falls back to the default with a warning instead of
/// silently disabling tracing.
#[cfg(feature = "otel")]
pub const TRACING_SAMPLE_RATIO_ENV: &str = "SDKWORK_TRACING_SAMPLE_RATIO";

#[cfg(feature = "otel")]
const DEFAULT_TRACING_SAMPLE_RATIO: f64 = 1.0;

/// Pure parser for the sampling ratio so the clamp/fallback policy is testable
/// without environment mutations. Returns the ratio to use.
#[cfg(feature = "otel")]
fn parse_tracing_sample_ratio(raw: Option<&str>) -> f64 {
    match raw {
        None => DEFAULT_TRACING_SAMPLE_RATIO,
        Some(value) => match value.trim().parse::<f64>() {
            Ok(ratio) if ratio.is_finite() => ratio.clamp(0.0, 1.0),
            _ => {
                tracing::warn!(
                    value = %value,
                    env = TRACING_SAMPLE_RATIO_ENV,
                    default = DEFAULT_TRACING_SAMPLE_RATIO,
                    "invalid tracing sample ratio; falling back to the default"
                );
                DEFAULT_TRACING_SAMPLE_RATIO
            }
        },
    }
}

#[cfg(feature = "otel")]
fn tracing_sample_ratio() -> f64 {
    parse_tracing_sample_ratio(std::env::var(TRACING_SAMPLE_RATIO_ENV).ok().as_deref())
}

pub fn init_tracing() {
    #[cfg(feature = "otel")]
    if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .ok()
        .is_some_and(|value| !is_blank(Some(value.as_str())))
    {
        match init_otel_tracing("sdkwork-api-memory-standalone-gateway") {
            Ok(()) => return,
            Err(error) => {
                eprintln!(
                    "sdkwork-api-memory-standalone-gateway OTLP tracing init failed ({error}); falling back to fmt subscriber"
                );
            }
        }
    }

    init_fmt_tracing();
}

fn init_fmt_tracing() {
    let environment =
        std::env::var("SDKWORK_MEMORY_ENVIRONMENT").unwrap_or_else(|_| "development".to_owned());
    let use_json = environment.eq_ignore_ascii_case("production")
        || std::env::var("SDKWORK_MEMORY_LOG_FORMAT")
            .map(|value| value.eq_ignore_ascii_case("json"))
            .unwrap_or(false);

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if use_json {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_current_span(true)
            .with_span_list(true)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }
}

#[cfg(feature = "otel")]
fn init_otel_tracing(service_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::SdkTracerProvider;
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")?;
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint)
        .build()?;
    // Head sampling: respect an upstream sampling decision, and sample root
    // traces by the configured ratio instead of always-on. Without
    // ParentBased wrapping, a ratio sampler would re-decide every span and
    // break traces whose upstream exporter already sampled a subset.
    let sampler = opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
        opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(tracing_sample_ratio()),
    ));
    // The sdk's `rt-tokio` feature binds the batch processor to the Tokio
    // runtime, so the exporter no longer takes a runtime argument.
    let provider = SdkTracerProvider::builder()
        .with_sampler(sampler)
        .with_batch_exporter(exporter)
        .build();
    let tracer = provider.tracer(service_name.to_owned());
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let environment =
        std::env::var("SDKWORK_MEMORY_ENVIRONMENT").unwrap_or_else(|_| "development".to_owned());
    let use_json = environment.eq_ignore_ascii_case("production")
        || std::env::var("SDKWORK_MEMORY_LOG_FORMAT")
            .map(|value| value.eq_ignore_ascii_case("json"))
            .unwrap_or(false);

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let fmt_layer = if use_json {
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(true)
            .with_span_list(true)
            .boxed()
    } else {
        tracing_subscriber::fmt::layer().boxed()
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(telemetry)
        .try_init()?;

    tracing::info!(
        service = service_name,
        "sdkwork-memory OTLP tracing initialized"
    );
    Ok(())
}

#[cfg(all(test, feature = "otel"))]
mod sample_ratio_tests {
    use super::parse_tracing_sample_ratio;

    #[test]
    fn sample_ratio_defaults_clamps_and_falls_back() {
        // Absent env keeps the always-sample default.
        assert_eq!(parse_tracing_sample_ratio(None), 1.0);
        // Explicit values pass through inside the valid range.
        assert_eq!(parse_tracing_sample_ratio(Some("0.25")), 0.25);
        assert_eq!(parse_tracing_sample_ratio(Some("1")), 1.0);
        // Out-of-range values clamp instead of being rejected.
        assert_eq!(parse_tracing_sample_ratio(Some("1.5")), 1.0);
        assert_eq!(parse_tracing_sample_ratio(Some("-3")), 0.0);
        // Unparseable values fall back to the default (with a warn log).
        assert_eq!(parse_tracing_sample_ratio(Some("half")), 1.0);
        assert_eq!(parse_tracing_sample_ratio(Some("")), 1.0);
        // Non-finite floats are rejected like any other garbage input.
        assert_eq!(parse_tracing_sample_ratio(Some("NaN")), 1.0);
        assert_eq!(parse_tracing_sample_ratio(Some("inf")), 1.0);
    }
}

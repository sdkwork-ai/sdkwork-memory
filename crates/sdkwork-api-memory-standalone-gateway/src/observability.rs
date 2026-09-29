//! Tracing bootstrap for `sdkwork-api-memory-standalone-gateway` (`OBSERVABILITY_SPEC.md` §2, §4).

#[cfg(feature = "otel")]
use sdkwork_utils_rust::is_blank;

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
    // The sdk's `rt-tokio` feature binds the batch processor to the Tokio
    // runtime, so the exporter no longer takes a runtime argument.
    let provider = SdkTracerProvider::builder()
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

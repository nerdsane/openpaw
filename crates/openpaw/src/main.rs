//! Open Paw — Agent daemon built on Temper platform.
//!
//! Boots an embedded Temper platform, installs Paw OS apps,
//! seeds agent souls, and starts the Discord transport.

mod config;
mod startup;

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let config = config::Config::from_env()?;

    // Build layered tracing subscriber
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,openpaw=debug".into());

    let fmt_layer = tracing_subscriber::fmt::layer();

    let _tracer_provider: Option<opentelemetry_sdk::trace::SdkTracerProvider> = if config.otel_enabled {
        let resource = opentelemetry_sdk::Resource::builder()
            .with_service_name("openpaw")
            .build();

        // Trace exporter → Datadog Agent via OTLP gRPC
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&config.otel_endpoint)
            .build()?;

        let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_batch_exporter(span_exporter)
            .with_resource(resource.clone())
            .build();

        // Metrics exporter → Datadog Agent via OTLP gRPC
        let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&config.otel_endpoint)
            .build()?;

        let metric_reader = opentelemetry_sdk::metrics::PeriodicReader::builder(metric_exporter)
            .build();

        let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
            .with_reader(metric_reader)
            .with_resource(resource)
            .build();

        opentelemetry::global::set_meter_provider(meter_provider);

        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer_provider.tracer("openpaw"));

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        tracing::info!("Open Paw starting (OpenTelemetry enabled → {})...", config.otel_endpoint);
        Some(tracer_provider)
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();

        tracing::info!("Open Paw starting...");
        None
    };

    let result = startup::run(config).await;

    // Flush pending spans on exit
    if let Some(provider) = _tracer_provider {
        let _ = provider.shutdown();
    }

    result
}

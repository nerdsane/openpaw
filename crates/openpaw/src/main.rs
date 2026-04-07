//! Open Paw — Agent daemon built on Temper platform.
//!
//! Boots an embedded Temper platform, installs Paw OS apps,
//! seeds agent souls, and starts the Discord transport.

mod config;
mod setup;
mod setup_api;
mod startup;
mod transport_manager;

use clap::{Parser, Subcommand};
use opentelemetry::trace::TracerProvider;
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::{
    BatchConfigBuilder as LogBatchConfigBuilder, BatchLogProcessor, SdkLoggerProvider,
};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{
    BatchConfigBuilder as SpanBatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider,
};
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const TRACE_BATCH_MAX_QUEUE_SIZE: usize = 2_048;
const TRACE_BATCH_MAX_EXPORT_BATCH_SIZE: usize = 512;
const TRACE_BATCH_SCHEDULE_DELAY_MS: u64 = 1_000;
const LOG_BATCH_MAX_QUEUE_SIZE: usize = 2_048;
const LOG_BATCH_MAX_EXPORT_BATCH_SIZE: usize = 512;
const LOG_BATCH_SCHEDULE_DELAY_MS: u64 = 1_000;

#[derive(Parser)]
#[command(name = "openpaw", about = "Open Paw — agent platform")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Configure API keys and messaging platforms interactively
    Setup,
    /// Diagnose configuration and show what's working
    Doctor,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = config::Config::from_env()?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = std::path::Path::new(&home).join(".local/share/openpaw");
    std::fs::create_dir_all(&data_dir)?;

    match cli.command {
        Some(Command::Setup) => {
            // Force-run setup regardless of current state
            let result = setup::run_setup(&data_dir, &config)?;
            setup::merge_setup_into_config(&mut config, result);
            println!("  Setup complete. Run `openpaw` to start the server.");
            return Ok(());
        }
        Some(Command::Doctor) => {
            setup::run_doctor(&data_dir, &config);
            return Ok(());
        }
        None => {
            // Default: boot server (with setup prompt if missing config and interactive terminal)
        }
    }

    // Build layered tracing subscriber
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,openpaw=debug".into());

    let fmt_layer = tracing_subscriber::fmt::layer();

    let mut resource_attrs = vec![opentelemetry::KeyValue::new(
        "service.name",
        "openpaw".to_string(),
    )];
    if let Ok(environment) = std::env::var("DD_ENV") {
        let environment = environment.trim();
        if !environment.is_empty() {
            resource_attrs.push(opentelemetry::KeyValue::new(
                "deployment.environment.name",
                environment.to_string(),
            ));
        }
    }
    if let Ok(version) = std::env::var("DD_VERSION") {
        let version = version.trim();
        if !version.is_empty() {
            resource_attrs.push(opentelemetry::KeyValue::new(
                "service.version",
                version.to_string(),
            ));
        }
    }

    let _providers: Option<(SdkTracerProvider, SdkMeterProvider, SdkLoggerProvider)> =
        if config.otel_enabled {
            let resource = opentelemetry_sdk::Resource::builder_empty()
                .with_attributes(resource_attrs)
                .build();

            // Trace exporter → Datadog Agent via OTLP gRPC
            let span_exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic()
                .with_endpoint(&config.otel_endpoint)
                .build()?;

            let trace_batch_config = SpanBatchConfigBuilder::default()
                .with_max_queue_size(TRACE_BATCH_MAX_QUEUE_SIZE)
                .with_max_export_batch_size(TRACE_BATCH_MAX_EXPORT_BATCH_SIZE)
                .with_scheduled_delay(Duration::from_millis(TRACE_BATCH_SCHEDULE_DELAY_MS))
                .build();

            let trace_batch_processor = BatchSpanProcessor::builder(span_exporter)
                .with_batch_config(trace_batch_config)
                .build();

            let tracer_provider = SdkTracerProvider::builder()
                .with_span_processor(trace_batch_processor)
                .with_resource(resource.clone())
                .build();

            // Metrics exporter → Datadog Agent via OTLP gRPC
            let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
                .with_tonic()
                .with_endpoint(&config.otel_endpoint)
                .build()?;

            let metric_reader = PeriodicReader::builder(metric_exporter).build();

            let meter_provider = SdkMeterProvider::builder()
                .with_reader(metric_reader)
                .with_resource(resource.clone())
                .build();

            opentelemetry::global::set_meter_provider(meter_provider.clone());

            // Logs exporter → Datadog Agent via OTLP gRPC
            let log_exporter = opentelemetry_otlp::LogExporter::builder()
                .with_tonic()
                .with_endpoint(&config.otel_endpoint)
                .build()?;

            let log_batch_config = LogBatchConfigBuilder::default()
                .with_max_queue_size(LOG_BATCH_MAX_QUEUE_SIZE)
                .with_max_export_batch_size(LOG_BATCH_MAX_EXPORT_BATCH_SIZE)
                .with_scheduled_delay(Duration::from_millis(LOG_BATCH_SCHEDULE_DELAY_MS))
                .build();

            let log_batch_processor = BatchLogProcessor::builder(log_exporter)
                .with_batch_config(log_batch_config)
                .build();

            let logger_provider = SdkLoggerProvider::builder()
                .with_log_processor(log_batch_processor)
                .with_resource(resource)
                .build();

            let otel_layer =
                tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer("openpaw"));
            let otel_log_layer = OpenTelemetryTracingBridge::new(&logger_provider);

            tracing_subscriber::registry()
                .with(env_filter)
                .with(fmt_layer)
                .with(otel_layer)
                .with(otel_log_layer)
                .init();

            tracing::info!(
                "Open Paw starting (OpenTelemetry enabled → {})...",
                config.otel_endpoint
            );
            Some((tracer_provider, meter_provider, logger_provider))
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
    if let Some((tracer_provider, meter_provider, logger_provider)) = _providers {
        let _ = tracer_provider.shutdown();
        let _ = meter_provider.shutdown();
        let _ = logger_provider.shutdown();
    }

    result
}

//! Internal logging and tracing configurations

use anyhow::Ok;
use opentelemetry::{TraceId, trace::TracerProvider};
use opentelemetry_resource_detectors::{K8sResourceDetector, ProcessResourceDetector};
use std::env;
use tracing_opentelemetry::OpenTelemetryLayer;

use opentelemetry_otlp::SpanExporter;

use opentelemetry::KeyValue;
use opentelemetry::trace::TraceContextExt as _;
use opentelemetry_sdk::{
    Resource,
    trace::{SdkTracer, SdkTracerProvider},
};
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::{
    EnvFilter, Layer, Registry, layer::SubscriberExt, util::SubscriberInitExt,
};

///  Fetch an opentelemetry::trace::TraceId as hex through the full tracing stack
pub fn get_trace_id() -> TraceId {
    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
}

fn resource() -> Resource {
    Resource::builder()
        .with_detector(Box::new(K8sResourceDetector))
        .with_detector(Box::new(ProcessResourceDetector))
        .with_service_name(env!("CARGO_PKG_NAME"))
        .with_attribute(KeyValue::new("service.version", env!("CARGO_PKG_VERSION")))
        .build()
}

fn init_tracer() -> anyhow::Result<SdkTracer> {
    let exporter = SpanExporter::builder().with_tonic().build()?;

    let provider = SdkTracerProvider::builder()
        .with_resource(resource())
        .with_batch_exporter(exporter)
        .build();

    Ok(provider.tracer("tracing-otel-subscriber"))
}

fn is_otel_enabled() -> bool {
    env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok()
}

/// Initializes tracing with subscribers
/// # Errors
/// Will return `Err` if it wasn't able to intialize tracing
pub fn init() -> anyhow::Result<()> {
    let logger = env::var("LOG_FORMAT").map_or(tracing_subscriber::fmt::layer().boxed(), |v| {
        if v == "json" {
            tracing_subscriber::fmt::layer().json().boxed()
        } else {
            tracing_subscriber::fmt::layer().boxed()
        }
    });

    let env_filter = EnvFilter::from_env("LOG_LEVEL");

    let reg = Registry::default().with(env_filter).with(logger);

    if is_otel_enabled() {
        let otel = OpenTelemetryLayer::new(init_tracer()?);
        reg.with(otel).try_init()?;
    } else {
        reg.try_init()?;
    }

    Ok(())
}

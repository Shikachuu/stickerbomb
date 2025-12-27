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

/// Fetch an `opentelemetry::trace::TraceId` as hex through the full tracing stack
#[must_use]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_otel_enabled_when_endpoint_set() {
        temp_env::with_var(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            Some("http://localhost:4317"),
            || {
                assert!(is_otel_enabled());
            },
        );
    }

    #[test]
    fn test_is_otel_disabled_when_endpoint_not_set() {
        temp_env::with_var_unset("OTEL_EXPORTER_OTLP_ENDPOINT", || {
            assert!(!is_otel_enabled());
        });
    }

    #[test]
    fn test_resource_contains_service_name() {
        let res = resource();
        let attrs: Vec<_> = res.iter().collect();

        // Check that service.name is set to the package name
        let service_name = attrs
            .iter()
            .find(|(k, _)| k.as_str() == "service.name")
            .map(|(_, v)| v.as_str());

        assert_eq!(service_name.as_deref(), Some(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn test_resource_contains_service_version() {
        let res = resource();
        let attrs: Vec<_> = res.iter().collect();

        // Check that service.version is set
        let service_version = attrs
            .iter()
            .find(|(k, _)| k.as_str() == "service.version")
            .map(|(_, v)| v.as_str());

        assert_eq!(service_version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_get_trace_id_with_otel_layer() {
        use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
        use tracing_subscriber::layer::SubscriberExt;

        let provider = SdkTracerProvider::builder()
            .with_sampler(Sampler::AlwaysOn)
            .build();

        let tracer = provider.tracer("test-tracer");
        let otel_layer = OpenTelemetryLayer::new(tracer);

        let subscriber = Registry::default().with(otel_layer);

        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("test_span");
            let _enter = span.enter();

            let trace_id = get_trace_id();

            assert_eq!(trace_id.to_string().len(), 32); // Trace ID should be 32 hex chars
            assert_ne!(trace_id.to_string(), "00000000000000000000000000000000");
        });
    }
}

use super::infer_protocol;
use opentelemetry_otlp::{ExporterBuildError, LogExporter, WithExportConfig};
use opentelemetry_sdk::logs::{
    BatchConfigBuilder, BatchLogProcessor, LoggerProviderBuilder, SdkLoggerProvider,
};
use opentelemetry_sdk::Resource;
use std::env;
use std::time::Duration;
#[cfg(feature = "tls")]
use {opentelemetry_otlp::WithTonicConfig, tonic::transport::ClientTlsConfig};

#[must_use]
pub fn identity(v: LoggerProviderBuilder) -> LoggerProviderBuilder {
    v
}

pub fn init_loggerprovider<F>(
    resource: Resource,
    transform: F,
) -> Result<SdkLoggerProvider, ExporterBuildError>
where
    F: FnOnce(LoggerProviderBuilder) -> LoggerProviderBuilder,
{
    let (maybe_protocol, maybe_endpoint) = read_protocol_and_endpoint_from_env();
    let protocol = infer_protocol(maybe_protocol.as_deref(), maybe_endpoint.as_deref());
    let timeout = env::var("OTEL_EXPORTER_OTLP_LOGS_TIMEOUT")
        .ok()
        .and_then(|var| var.parse::<u64>().ok())
        .map_or(Duration::from_secs(10), Duration::from_secs);

    let exporter = match protocol.as_deref() {
        Some("http/protobuf") => Some(
            LogExporter::builder()
                .with_http()
                .with_timeout(timeout)
                .build()?,
        ),
        #[cfg(feature = "tls")]
        Some("grpc/tls") => Some(
            LogExporter::builder()
                .with_tonic()
                .with_tls_config(ClientTlsConfig::new().with_enabled_roots())
                .with_timeout(timeout)
                .build()?,
        ),
        Some("grpc") => Some(
            LogExporter::builder()
                .with_tonic()
                .with_timeout(timeout)
                .build()?,
        ),
        Some(x) => {
            tracing::warn!(
                "unknown '{x}' env var set or infered for OTEL_EXPORTER_OTLP_LOGS_PROTOCOL or OTEL_EXPORTER_OTLP_PROTOCOL; no log exporter will be created"
            );
            None
        }
        None => {
            tracing::warn!(
                "no env var set or infered for OTEL_EXPORTER_OTLP_LOGS_PROTOCOL or OTEL_EXPORTER_OTLP_PROTOCOL; no log exporter will be created"
            );
            None
        }
    };
    let mut logger_provider = SdkLoggerProvider::builder().with_resource(resource);
    if let Some(exporter) = exporter {
        // let processor = BatchLogProcessor::builder(exporter)
        //     .with_batch_config(BatchConfigBuilder::default().build())
        //     .build();
        // logger_provider = logger_provider.with_log_processor(processor);
        logger_provider = logger_provider.with_batch_exporter(exporter);
    }
    logger_provider = transform(logger_provider);
    Ok(logger_provider.build())
}

fn read_protocol_and_endpoint_from_env() -> (Option<String>, Option<String>) {
    let maybe_protocol = std::env::var("OTEL_EXPORTER_OTLP_LOGS_PROTOCOL")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL"))
        .ok();
    let maybe_endpoint = std::env::var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT")
        .or_else(|_| {
            std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").map(|endpoint| match &maybe_protocol {
                Some(protocol) if protocol.contains("http") => {
                    format!("{endpoint}/v1/logs")
                }
                _ => endpoint,
            })
        })
        .ok();
    (maybe_protocol, maybe_endpoint)
}

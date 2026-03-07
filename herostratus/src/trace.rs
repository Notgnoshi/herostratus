use tracing_subscriber::filter::filter_fn;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

/// Initialize the tracing subscriber with an optional Chrome trace layer.
///
/// The fmt layer logs to stderr with the given log level and color settings, filtering out spans
/// with `target: "perf"` so that performance instrumentation never appears in diagnostic logs.
///
/// When the `HEROSTRATUS_TRACE` environment variable is set (to anything),
/// [ChromeLayer](tracing_chrome::ChromeLayer) is added that writes Chrome Trace Event Format JSON
/// to `herostratus-{unix_timestamp}.json` in the current working directory. The returned
/// [FlushGuard](tracing_chrome::FlushGuard) must be held until the end of the program to ensure
/// the trace file is flushed.
pub fn init(log_level: tracing::Level, use_color: bool) -> Option<tracing_chrome::FlushGuard> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(log_level.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .with_filter(env_filter)
        .with_filter(filter_fn(|metadata| metadata.target() != "perf"));

    let (chrome_layer, guard) = if std::env::var("HEROSTRATUS_TRACE").as_deref().is_ok() {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_secs();
        let filename = format!("herostratus-{timestamp}.json");
        let (layer, guard) = tracing_chrome::ChromeLayerBuilder::new()
            .file(filename)
            .include_args(true)
            .include_locations(true)
            .build();
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(fmt_layer)
        .with(chrome_layer)
        .init();

    guard
}

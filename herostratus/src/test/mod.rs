pub mod fixtures;

use tracing::Level;
use tracing_subscriber::fmt::writer::TestWriter;
use tracing_subscriber::EnvFilter;

#[ctor::ctor]
fn setup_test_logging() {
    let filter = EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())
        .with_env_var("HEROSTRATUS_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(TestWriter::new())
        .init();
}

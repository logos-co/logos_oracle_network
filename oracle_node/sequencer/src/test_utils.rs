use std::sync::Once;
use tracing_subscriber::EnvFilter;

static TRACING_INIT: Once = Once::new();

pub fn setup_test_tracing() {
    TRACING_INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    });
}
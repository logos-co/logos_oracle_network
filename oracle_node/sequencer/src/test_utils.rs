use tracing_subscriber::EnvFilter;

pub fn setup_test_tracing() {
    // Note: tracing-test crate is another dev dependency and only listen to main thread
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter(EnvFilter::from_default_env())
        // ignore error if another // unit test already init. it
        .try_init();
}
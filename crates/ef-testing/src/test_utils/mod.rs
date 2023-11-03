use std::sync::Once;

use tracing_subscriber::{filter, FmtSubscriber};

static INIT: Once = Once::new();

pub fn setup() {
    INIT.call_once(|| {
        // Set-up tracing filter
        let filter = filter::EnvFilter::new("ef_testing=info,sequencer=info");
        let subscriber = FmtSubscriber::builder()
            .with_env_filter(filter)
            .without_time()
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    })
}

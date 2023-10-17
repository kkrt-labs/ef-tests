use std::{str::FromStr, sync::Once};

use ef_testing::models::case::BlockchainTestCase;
use ef_testing::traits::Case;
use ef_testing_proc_macro::generate_blockchain_tests;
use ef_tests::models::{Block, RootOrState, State};
use revm_primitives::B256;
use tracing_subscriber::{filter, FmtSubscriber};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        // Set-up tracing filter
        let filter = filter::EnvFilter::new("ef_testing=info,sequencer=warn");
        let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    })
}

generate_blockchain_tests!();

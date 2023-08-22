#![cfg(feature = "ef-tests")]

use ef_testing::models::case::BlockchainTestSuite;
use ef_tests::Suite;
use kakarot_rpc_core::test_utils::deploy_helpers::KakarotTestEnvironmentContext;
use kakarot_rpc_core::test_utils::fixtures::kakarot_test_env_ctx;
use rstest::rstest;

macro_rules! blockchain_tests {
    ($test_name:ident, $dir:ident) => {
        #[rstest]
        #[tokio::test(flavor = "multi_thread")]
        async fn $test_name(kakarot_test_env_ctx: KakarotTestEnvironmentContext) {
            BlockchainTestSuite::new(format!("GeneralStateTests/{}", stringify!($dir))).run();
        }
    };
}

mod blockchain_tests {
    use super::*;

    blockchain_tests!(vm_tests, VMTests);
}

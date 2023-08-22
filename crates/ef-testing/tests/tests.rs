#![cfg(feature = "ef-tests")]

use ef_testing::models::suite::BlockchainTestSuite;
use ef_tests::Suite;
use rstest::rstest;

macro_rules! blockchain_tests {
    ($test_name:ident, $dir:ident) => {
        #[tokio::test]
        async fn $test_name() {
            BlockchainTestSuite::new(
                format!("GeneralStateTests/{}", stringify!($dir)),
                kakarot_test_env_ctx,
            )
            .run()
            .await;
        }
    };
}

mod blockchain_tests {
    use super::*;

    blockchain_tests!(vm_tests, VMTests);
}

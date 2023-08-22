#![cfg(feature = "ef-tests")]

use ef_testing::models::suite::BlockchainTestSuite;
use ef_testing::traits::Suite;

macro_rules! blockchain_tests {
    ($test_name:ident, $dir:ident) => {
        #[tokio::test]
        async fn $test_name() {
            BlockchainTestSuite::new(format!("GeneralStateTests/{}", stringify!($dir)))
                .run()
                .await;
        }
    };
}

mod blockchain_tests {
    use super::*;
    use ctor::ctor;
    use tracing_subscriber::FmtSubscriber;

    #[ctor]
    fn setup() {
        let subscriber = FmtSubscriber::builder()
            .with_max_level(tracing::Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");
    }

    blockchain_tests!(vm_tests, VMTests);
}

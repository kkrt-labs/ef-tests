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
    use std::{format, fs};
    use tracing_subscriber::{filter, FmtSubscriber};

    pub fn verify_kakarot_sha() -> Result<String, eyre::Error> {
        let remote_sha = fs::read_to_string("../../.katana/remote_kakarot_sha")?;
        let local_sha = fs::read_to_string("../../.katana/kakarot_sha")?;

        if remote_sha != local_sha {
            return Err(eyre::eyre!(format!(
                "Kakarot commit hash mismatch: local: {}, remote: {}",
                local_sha, remote_sha
            )));
        }

        Ok(remote_sha)
    }

    #[ctor]
    fn setup() {
        // Set-up tracing filter
        let filter =
            filter::EnvFilter::new("tests::blockchain_tests=info,ef_testing=info,katana_core=info");
        let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("setting tracing default failed");

        match verify_kakarot_sha() {
            Ok(sha) => {
                tracing::info!("Using Kakarot sha {}", sha);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to verify Kakarot sha: {}. Pull latest dump with `make fetch-dump`",
                    e
                );
            }
        };
    }

    blockchain_tests!(vm_tests, VMTests);
}

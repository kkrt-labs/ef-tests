#![cfg(feature = "ef-tests")]

use ef_testing::models::suite::BlockchainTestSuite;
use ef_testing::traits::Suite;
use std::sync::Once;
use std::{format, fs};
use tracing_subscriber::{filter, FmtSubscriber};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        // Set-up tracing filter
        let filter = filter::EnvFilter::new("tests=info,ef_testing=info,katana_core=info");
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
    })
}

pub fn verify_kakarot_sha() -> Result<String, eyre::Error> {
    // This is the SHA hash of the latest Kakarot submodule commit, inside Kakarot-RPC
    let remote_sha = fs::read_to_string("../../.katana/remote_kakarot_sha")?;
    // This is your local SHA hash of the Kakarot commit currently used in the dump.
    let local_sha = fs::read_to_string("../../.katana/kakarot_sha")?;

    // Helper check to remind you to locally run `make fetch-dump` often
    if remote_sha != local_sha {
        return Err(eyre::eyre!(format!(
            "Kakarot commit hash mismatch: local: {}, remote (kakarot submodule in kakarot-rpc repository): {}",
            local_sha, remote_sha
        )));
    }

    Ok(remote_sha)
}

macro_rules! blockchain_tests {
    ($test_name:ident, $dir:ident) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $test_name() {
            setup();
            BlockchainTestSuite::new(format!("GeneralStateTests/{}", stringify!($dir)))
                .run()
                .await;
        }
    };
}

mod blockchain_tests {
    use super::*;

    blockchain_tests!(st_sload, stSLoadTest);
    blockchain_tests!(vm_tests, VMTests);
    blockchain_tests!(memory_tests, stMemoryTest);
    blockchain_tests!(init_code_test, stInitCodeTest);
    blockchain_tests!(st_solidity_test, stSolidityTest);
    blockchain_tests!(st_log_tests, stLogTests);
    blockchain_tests!(st_bad_opcode, stBadOpcode);
    blockchain_tests!(shanghai, Shanghai);
    blockchain_tests!(st_bugs, stBugs);
    blockchain_tests!(st_call_codes, stCallCodes);
    //blockchain_tests!(st_call_create_call_code_test, stCallCreateCallCodeTest); // ef-tests #257
    blockchain_tests!(
        st_call_delegate_codes_call_code_homestead,
        stCallDelegateCodesCallCodeHomestead
    );
    blockchain_tests!(
        st_call_delegate_codes_homestead,
        stCallDelegateCodesHomestead
    );
    blockchain_tests!(st_delegatecall_test_homestead, stDelegatecallTestHomestead);
}

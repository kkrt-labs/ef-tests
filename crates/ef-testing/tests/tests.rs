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
        let filter = filter::EnvFilter::new("tests=warn,ef_testing=warn,executor=warn");
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

    // TODO: Uncomment tests to add entire folder to test flow.
    // All failing tests should be added to the skip session at "blockchain-tests-skip.yml"
    // and a respective issue at https://github.com/kkrt-labs/ef-tests/issues should be created.
    // Tests from the same folder with the same error can be aggregated in one issue

    blockchain_tests!(shanghai, Shanghai);
    blockchain_tests!(st_args_zero_one_balance, stArgsZeroOneBalance);
    blockchain_tests!(st_attack_test, stAttackTest);
    blockchain_tests!(st_bad_opcode, stBadOpcode);
    blockchain_tests!(st_bugs, stBugs);
    blockchain_tests!(st_call_codes, stCallCodes);
    // blockchain_tests!(st_call_create_call_code_test, stCallCreateCallCodeTest); // ef-tests #257
    blockchain_tests!(
        st_call_delegate_codes_call_code_homestead,
        stCallDelegateCodesCallCodeHomestead
    );
    blockchain_tests!(
        st_call_delegate_codes_homestead,
        stCallDelegateCodesHomestead
    );
    blockchain_tests!(st_chain_id, stChainId);
    blockchain_tests!(st_code_copy_test, stCodeCopyTest);
    blockchain_tests!(st_code_size_limit, stCodeSizeLimit);
    blockchain_tests!(st_create2, stCreate2);
    blockchain_tests!(st_create_test, stCreateTest);
    blockchain_tests!(st_delegatecall_test_homestead, stDelegatecallTestHomestead);
    blockchain_tests!(st_eip150single_code_gas_prices, stEIP150singleCodeGasPrices);
    blockchain_tests!(st_eip150_specific, stEIP150Specific);
    blockchain_tests!(st_eip158_specific, stEIP158Specific);
    // blockchain_tests!(st_eip1559, stEIP1559); // ef-tests #455 - failing due to missing field gasPrice, cannot measure number of failing tests
    // blockchain_tests!(st_eip2930, stEIP2930); // ef-tests #455 - failing due to missing field gasPrice, cannot measure number of failing tests
    blockchain_tests!(st_eip3607, stEIP3607);
    // blockchain_tests!(st_example, stExample); // ef-tests #455 - failing due to missing field gasPrice, cannot measure number of failing tests
    blockchain_tests!(st_ext_code_hash, stExtCodeHash);
    blockchain_tests!(st_homestead_specific, stHomesteadSpecific);
    blockchain_tests!(st_init_code_test, stInitCodeTest);
    blockchain_tests!(st_log_tests, stLogTests);
    blockchain_tests!(st_mem_expanding_eip150_calls, stMemExpandingEIP150Calls);
    blockchain_tests!(st_memory_stress_test, stMemoryStressTest);
    blockchain_tests!(st_memory_test, stMemoryTest);
    blockchain_tests!(st_non_zero_calls_test, stNonZeroCallsTest);
    blockchain_tests!(st_pre_compiled_contracts, stPreCompiledContracts);
    blockchain_tests!(st_pre_compiled_contracts2, stPreCompiledContracts2);
    blockchain_tests!(st_quadratic_complexity_test, stQuadraticComplexityTest);
    // blockchain_tests!(st_random, stRandom); // 79 failing tests
    // blockchain_tests!(st_random2, stRandom2); // 40 failing tests
    blockchain_tests!(st_recursive_create, stRecursiveCreate);
    blockchain_tests!(st_refund_test, stRefundTest);
    blockchain_tests!(st_return_data_test, stReturnDataTest);
    blockchain_tests!(st_revert_test, stRevertTest);
    blockchain_tests!(st_self_balance, stSelfBalance);
    blockchain_tests!(st_shift, stShift);
    blockchain_tests!(st_sload_test, stSLoadTest);
    blockchain_tests!(st_solidity_test, stSolidityTest);
    blockchain_tests!(st_special_test, stSpecialTest);
    blockchain_tests!(st_sstore_test, stSStoreTest);
    blockchain_tests!(st_stack_tests, stStackTests);
    blockchain_tests!(st_static_call, stStaticCall);
    blockchain_tests!(st_static_flag_enabled, stStaticFlagEnabled);
    blockchain_tests!(st_system_operations_test, stSystemOperationsTest);
    // blockchain_tests!(st_time_consuming, stTimeConsuming); // 14 failing tests
    // blockchain_tests!(st_transaction_test, stTransactionTest); // failing due to: invalid length 62, expected a (both 0x-prefixed or not) hex string or byte array containing betwee
    blockchain_tests!(st_transition_test, stTransitionTest);
    blockchain_tests!(st_wallet_test, stWalletTest);
    blockchain_tests!(st_zero_calls_revert, stZeroCallsRevert);
    blockchain_tests!(st_zero_knowledge, stZeroKnowledge);
    blockchain_tests!(st_zero_knowledge2, stZeroKnowledge2);
    blockchain_tests!(st_zero_calls_test, stZeroCallsTest);
    blockchain_tests!(vm_tests, VmTests);
}

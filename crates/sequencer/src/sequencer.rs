use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::{
    commit::Committer,
    execution::{Execution, TransactionExecutionResult},
};
use cairo_native::cache::ProgramCache;
use starknet_in_rust::{
    definitions::block_context::BlockContext,
    execution::TransactionExecutionInfo,
    state::{
        cached_state::CachedState,
        contract_class_cache::PermanentContractClassCache,
        state_api::{State, StateReader},
    },
    transaction::{ClassHash, Transaction},
};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
/// We bound S to implement State .
#[derive(Clone, Default, Debug)]
pub struct Sequencer<S>
where
    S: State + StateReader,
{
    pub block_context: BlockContext,
    pub state: S,
}

impl<S> Sequencer<S>
where
    S: State + StateReader,
{
    /// Creates a new Sequencer instance.
    #[inline]
    #[must_use]
    pub const fn new(block_context: BlockContext, state: S) -> Self {
        Self {
            block_context,
            state,
        }
    }

    /// Returns the block context.
    #[inline]
    pub fn block_context(&mut self) -> BlockContext {
        self.block_context.clone()
    }
}

impl<S> Execution for Sequencer<S>
where
    S: Clone + State + StateReader + Committer,
{
    /// Executes the provided transaction on the current state and leads to a commitment of the
    /// cached state in the case of success. Reversion of the transaction leads to a discarding
    /// of the cached state but still increments the nonce of the sender.
    fn execute(
        &mut self,
        transaction: &Transaction,
        cache: Option<Rc<RefCell<ProgramCache<'_, ClassHash>>>>,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let sender_address = transaction.contract_address();

        let state_reader = self.state.clone();
        let mut cached_state = CachedState::new(
            Arc::new(state_reader),
            Arc::new(PermanentContractClassCache::default()),
        );

        let res = transaction.execute(&mut cached_state, &self.block_context, u128::MAX, cache);

        let execution_information = match res {
            Err(err) => {
                return Err(err);
            }
            Ok(execution_information) => {
                if execution_information.revert_error.is_some() {
                    // If the transaction reverted, we increment the nonce.
                    self.state.increment_nonce(&sender_address)?;
                } else {
                    // If the transaction succeeded, we commit the state.
                    self.state.commit(&mut cached_state)?;
                }
                execution_information
            }
        };

        Ok(execution_information)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;

    use cairo_lang_starknet::contract_class::ContractClass as SierraContractClass;
    use cairo_native::cache::AotProgramCache;
    use cairo_native::OptLevel;
    use starknet::core::types::contract::legacy::LegacyContractClass;
    use starknet::macros::selector;
    use starknet_in_rust::definitions::block_context::{GasPrices, StarknetOsConfig};
    use starknet_in_rust::definitions::constants::EXECUTE_ENTRY_POINT_SELECTOR;
    use starknet_in_rust::services::api::contract_classes::compiled_class::CompiledClass;
    use starknet_in_rust::services::api::contract_classes::deprecated_contract_class::ContractClass;
    use starknet_in_rust::state::state_api::State as _;
    use starknet_in_rust::state::BlockInfo;
    use starknet_in_rust::transaction::{Address, InvokeFunction, VersionSpecificAccountTxFields};
    use starknet_in_rust::utils::{
        field_element_to_felt, get_native_context, get_storage_var_address,
    };
    use starknet_in_rust::CasmContractClass;
    use starknet_in_rust::Felt252;

    use super::*;
    use crate::constants::test_constants::{
        CHAIN_ID, ETH_FEE_TOKEN_ADDRESS, FEE_TOKEN_ADDRESSES, ONE_CLASS_HASH, SEQUENCER_ADDRESS,
        TWO_CLASS_HASH, ZERO,
    };
    use crate::constants::test_constants::{TEST_ACCOUNT, TEST_CONTRACT};
    use crate::state::State;

    enum Version {
        V0,
        V1,
        Native,
    }

    fn read_contract_class_v0(path: &Path) -> CompiledClass {
        let s = std::fs::read_to_string(path).expect("Failed to read v0 contract class");
        let legacy_contract_class = serde_json::from_str::<LegacyContractClass>(&s).unwrap();
        let class_hash = legacy_contract_class
            .class_hash()
            .expect("Failed to get class hash");

        let contract_class = ContractClass::from_program_json_and_class_hash(
            &s,
            Felt252::from_bytes_be(&class_hash.to_bytes_be()),
        )
        .expect("Failed to get contract class");
        CompiledClass::Deprecated(Arc::new(contract_class))
    }

    fn read_contract_class_v1(path: &Path) -> CompiledClass {
        let s = std::fs::read_to_string(path).expect("Failed to read native contract class");
        let contract_class = serde_json::from_str::<SierraContractClass>(&s)
            .expect("Failed to parse contract class");

        let casm_contract_class = CasmContractClass::from_contract_class(contract_class, true)
            .expect("Failed to get casm contract class");
        CompiledClass::Casm {
            casm: Arc::new(casm_contract_class),
            sierra: None,
        }
    }

    fn read_contract_class_native(path: &Path) -> CompiledClass {
        let s = std::fs::read_to_string(path).expect("Failed to read native contract class");
        let contract_class = serde_json::from_str::<SierraContractClass>(&s)
            .expect("Failed to parse contract class");

        let sierra_program = contract_class.extract_sierra_program().unwrap();
        let entrypoints = contract_class.entry_points_by_type.clone();
        let casm_contract_class = CasmContractClass::from_contract_class(contract_class, true)
            .expect("Failed to get casm contract class");

        CompiledClass::Casm {
            casm: Arc::new(casm_contract_class),
            sierra: Some(Arc::new((sierra_program, entrypoints))),
        }
    }

    fn declare_and_deploy_contract(
        path: &Path,
        address: Address,
        class_hash: ClassHash,
        state: &mut State,
        version: Version,
        cache: Option<Rc<RefCell<ProgramCache<'_, ClassHash>>>>,
    ) {
        let contract_class = match version {
            Version::V0 => read_contract_class_v0(path),
            Version::V1 => read_contract_class_v1(path),
            Version::Native => {
                let compiled_class = read_contract_class_native(path);
                let program = match &compiled_class {
                    CompiledClass::Casm {
                        sierra: Some(program),
                        ..
                    } => program.0.clone(),
                    _ => unreachable!("Should not be deprecated"),
                };
                if let Some(cache) = cache {
                    let cache = &mut *cache.borrow_mut();
                    match cache {
                        ProgramCache::Aot(cache) => {
                            cache.compile_and_insert(class_hash, &program, OptLevel::Aggressive);
                        }
                        ProgramCache::Jit(cache) => {
                            cache.compile_and_insert(class_hash, &program, OptLevel::Aggressive);
                        }
                    }
                }
                compiled_class
            }
        };

        state
            .set_contract_class(&class_hash, &contract_class)
            .unwrap();
        state.set_class_hash_at(address, class_hash).unwrap();
        state
            .set_compiled_class_hash(
                &Felt252::from_bytes_be(&class_hash.as_slice()),
                &Felt252::from_bytes_be(&class_hash.as_slice()),
            )
            .unwrap();
    }

    fn fund(address: &Address, state: &mut State) {
        state.set_storage_at(
            &(
                ETH_FEE_TOKEN_ADDRESS.clone(),
                get_storage_var_address("ERC20_balances", &[address.0])
                    .expect("Failed to get storage var address")
                    .to_bytes_be(),
            ),
            Felt252::from(u128::MAX),
        );
    }

    fn block_context() -> BlockContext {
        let starknet_os_config =
            StarknetOsConfig::new(*CHAIN_ID, FEE_TOKEN_ADDRESSES.clone(), GasPrices::default());
        let block_info = BlockInfo {
            block_number: 0,
            block_timestamp: 0,
            gas_price: GasPrices::default(),
            sequencer_address: SEQUENCER_ADDRESS.clone(),
        };
        BlockContext::new(
            starknet_os_config,
            10,
            10,
            vm_resource_fee_cost(),
            4_000_000,
            4_000_000,
            block_info,
            HashMap::new(),
            false,
        )
    }

    /// Maps builtins and steps to a single cost unit of reference (gas).
    fn vm_resource_fee_cost() -> HashMap<String, f64> {
        [
            ("n_steps".to_string(), 1_f64),
            ("pedersen_builtin".to_string(), 1_f64),
            ("range_check_builtin".to_string(), 1_f64),
            ("ecdsa_builtin".to_string(), 1_f64),
            ("bitwise_builtin".to_string(), 1_f64),
            ("poseidon_builtin".to_string(), 1_f64),
            ("output_builtin".to_string(), 1_f64),
            ("ec_op_builtin".to_string(), 1_f64),
            ("keccak_builtin".to_string(), 1_f64),
            ("segment_arena_builtin".to_string(), 1_f64),
        ]
        .into_iter()
        .collect()
    }

    fn test_transaction(nonce: &Felt252) -> Transaction {
        let skip_validation = false;
        let skip_execute = false;
        let skip_fee_transfer = true;
        let ignore_max_fee = true;
        let skip_nonce_check = false;
        Transaction::InvokeFunction(
            InvokeFunction::new(
                TEST_ACCOUNT.clone(),
                *EXECUTE_ENTRY_POINT_SELECTOR,
                VersionSpecificAccountTxFields::Deprecated(1_000_000),
                Felt252::from(1),
                vec![
                    TEST_CONTRACT.0, // destination
                    field_element_to_felt(&selector!("inc")),
                    *ZERO, // no data
                ],
                vec![],
                *CHAIN_ID,
                Some(*nonce),
            )
            .expect("Failed to create transaction"),
        )
        .create_for_simulation(
            skip_validation,
            skip_execute,
            skip_fee_transfer,
            ignore_max_fee,
            skip_nonce_check,
        )
    }

    #[test]
    fn test_sequencer_cairo_0() {
        // Given
        let mut state = State::default();
        let mutable = &mut state;

        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_0/compiled_classes/counter.json"),
            TEST_CONTRACT.clone(),
            *ONE_CLASS_HASH,
            mutable,
            Version::V0,
            None,
        );
        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_0/compiled_classes/account.json"),
            TEST_ACCOUNT.clone(),
            *TWO_CLASS_HASH,
            mutable,
            Version::V0,
            None,
        );
        fund(&TEST_ACCOUNT, mutable);

        let context = block_context();
        let mut sequencer = Sequencer::new(context, state);

        // When
        let transaction = test_transaction(&Felt252::ZERO);
        sequencer.execute(&transaction, None).unwrap();

        // Then
        let expected = Felt252::from(1u8);
        let actual = sequencer
            .state
            .get_storage_at(&(
                TEST_CONTRACT.clone(),
                get_storage_var_address("counter", &[])
                    .unwrap()
                    .to_bytes_be(),
            ))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_sequencer_cairo_1() {
        // Given
        let mut state = State::default();

        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_1/compiled_classes/counter.json"),
            TEST_CONTRACT.clone(),
            *ONE_CLASS_HASH,
            &mut state,
            Version::V1,
            None,
        );
        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_1/compiled_classes/account.json"),
            TEST_ACCOUNT.clone(),
            *TWO_CLASS_HASH,
            &mut state,
            Version::V1,
            None,
        );
        fund(&TEST_ACCOUNT, &mut state);

        let context = block_context();
        let mut sequencer = Sequencer::new(context, state);

        // When
        let transaction = test_transaction(&Felt252::ZERO);
        sequencer.execute(&transaction, None).unwrap();

        // Then
        let expected = Felt252::from(1u8);
        let actual = sequencer
            .state
            .get_storage_at(&(
                TEST_CONTRACT.clone(),
                get_storage_var_address("counter", &[])
                    .unwrap()
                    .to_bytes_be(),
            ))
            .unwrap();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_sequencer_cairo_1_native() {
        // Given
        let mut state = State::default();
        std::env::set_var(
            "CAIRO_NATIVE_RUNTIME_LIBDIR",
            "~/code/rust/cairo_native/target/release",
        );

        let cache = Rc::new(RefCell::new(ProgramCache::Aot(AotProgramCache::new(
            get_native_context(),
        ))));
        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_1/compiled_classes/counter.json"),
            TEST_CONTRACT.clone(),
            *ONE_CLASS_HASH,
            &mut state,
            Version::Native,
            Some(Rc::clone(&cache)),
        );
        declare_and_deploy_contract(
            Path::new("src/test_data/cairo_1/compiled_classes/account.json"),
            TEST_ACCOUNT.clone(),
            *TWO_CLASS_HASH,
            &mut state,
            Version::Native,
            Some(Rc::clone(&cache)),
        );
        fund(&TEST_ACCOUNT, &mut state);

        let context = block_context();
        let mut sequencer = Sequencer::new(context, state);

        // When
        let transaction = test_transaction(&Felt252::ZERO);
        sequencer.execute(&transaction, Some(cache)).unwrap();

        // Then
        let expected = Felt252::from(1u8);
        let actual = sequencer
            .state
            .get_storage_at(&(
                TEST_CONTRACT.clone(),
                get_storage_var_address("counter", &[])
                    .unwrap()
                    .to_bytes_be(),
            ))
            .unwrap();
        assert_eq!(expected, actual);
    }
}

use crate::{commit::Committer, execution::Execution};
use blockifier::{
    context::BlockContext,
    state::{
        cached_state::CachedState,
        state_api::{State, StateReader},
    },
    transaction::{
        objects::{TransactionExecutionInfo, TransactionExecutionResult},
        transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
use starknet_api::core::ContractAddress;

/// Sequencer is the main struct of the sequencer crate.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Sequencer<S, A> {
    pub(crate) block_context: BlockContext,
    pub(crate) state: S,
    pub(crate) address: A,
}

impl<S, A> Sequencer<S, A> {
    /// Creates a new Sequencer instance.
    #[inline]
    #[must_use]
    pub const fn new(block_context: BlockContext, state: S, address: A) -> Self {
        Self {
            block_context,
            state,
            address,
        }
    }

    pub const fn block_context(&self) -> &BlockContext {
        &self.block_context
    }

    /// Returns a reference to the state.
    pub const fn state(&self) -> &S {
        &self.state
    }

    /// Returns a mutable reference to the state.
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }

    /// Returns the address of the sequencer.
    pub const fn address(&self) -> &A {
        &self.address
    }
}

/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
/// We bound S such that a mutable reference to S (&'a mut S)
/// must implement State and `StateReader`. The `for` keyword
/// indicates that the bound must hold for any lifetime 'any.
/// For more details, check out [rust-lang docs](https://doc.rust-lang.org/nomicon/hrtb.html)
impl<S, A> Execution for Sequencer<S, A>
where
    for<'any> &'any mut S: State + StateReader + Committer<S>,
{
    /// Executes the provided transaction on the current state and leads to a commitment of the
    /// cached state in the case of success. Reversion of the transaction leads to a discarding
    /// of the cached state but still increments the nonce of the sender.
    fn execute(
        &mut self,
        transaction: Transaction,
    ) -> TransactionExecutionResult<TransactionExecutionInfo> {
        let sender_address = match &transaction {
            Transaction::AccountTransaction(tx) => match tx {
                blockifier::transaction::account_transaction::AccountTransaction::Invoke(tx) => {
                    tx.tx.sender_address()
                }
                blockifier::transaction::account_transaction::AccountTransaction::Declare(tx) => {
                    tx.tx().sender_address()
                }
                blockifier::transaction::account_transaction::AccountTransaction::DeployAccount(
                    tx,
                ) => tx.contract_address,
            },
            Transaction::L1HandlerTransaction(_) => ContractAddress::from(0u8),
        };

        let mut cached_state = CachedState::new(&mut self.state);
        let charge_fee = false;
        let validate = true;
        let res = transaction.execute(&mut cached_state, &self.block_context, charge_fee, validate);

        let execution_information = match res {
            Err(err) => {
                return Err(err);
            }
            Ok(execution_information) => {
                if execution_information.revert_error.is_some() {
                    // If the transaction reverted, we increment the nonce.
                    (&mut self.state).increment_nonce(sender_address)?;
                } else {
                    // If the transaction succeeded, we commit the state.
                    <&mut S>::commit(&mut cached_state)?;
                }
                execution_information
            }
        };

        Ok(execution_information)
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Display;
    use std::fs::File;
    use std::num::NonZeroU128;

    use blockifier::abi::abi_utils::get_storage_var_address;
    use blockifier::blockifier::block::{BlockInfo, GasPrices};
    use blockifier::context::ChainInfo;
    use blockifier::context::{BlockContext, FeeTokenAddresses};
    use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
    use blockifier::state::state_api::State as BlockifierState;
    use blockifier::transaction::account_transaction::AccountTransaction;
    use blockifier::transaction::transactions::InvokeTransaction as BlockifierInvokeTransaction;
    use blockifier::versioned_constants::VersionedConstants;
    use starknet::macros::selector;
    use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
    use starknet_api::hash::StarkFelt;
    use starknet_api::transaction::{
        Calldata, Fee, InvokeTransaction, InvokeTransactionV1, TransactionHash,
        TransactionSignature,
    };

    use crate::constants::test_constants::{
        ETH_FEE_TOKEN_ADDRESS, SEQUENCER_ADDRESS, STRK_FEE_TOKEN_ADDRESS,
    };
    use crate::constants::test_constants::{
        ONE_BLOCK_NUMBER, ONE_BLOCK_TIMESTAMP, ONE_CLASS_HASH, TEST_ACCOUNT, TEST_CONTRACT,
        TWO_CLASS_HASH, ZERO_FELT,
    };
    use crate::state::State;

    use super::*;

    enum CairoVersion {
        V0,
        V1,
    }

    impl Display for CairoVersion {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::V0 => write!(f, "cairo_0"),
                Self::V1 => write!(f, "cairo_1"),
            }
        }
    }

    fn read_contract_class_v0(path: &str) -> ContractClass {
        let reader = File::open(path).unwrap();
        let contract_class: ContractClassV0 = serde_json::from_reader(reader).unwrap();

        ContractClass::V0(contract_class)
    }

    fn read_contract_class_v1(path: &str) -> ContractClass {
        let raw_contract_class = std::fs::read_to_string(path).unwrap();
        let contract_class: ContractClassV1 =
            ContractClassV1::try_from_json_string(&raw_contract_class).unwrap();

        ContractClass::V1(contract_class)
    }

    fn declare_and_deploy_contract(
        path: &str,
        address: ContractAddress,
        class_hash: ClassHash,
        mut state: &mut State,
        version: CairoVersion,
    ) {
        let contract_class = match version {
            CairoVersion::V0 => read_contract_class_v0(path),
            CairoVersion::V1 => read_contract_class_v1(path),
        };

        state
            .set_contract_class(class_hash, contract_class)
            .unwrap();
        state.set_class_hash_at(address, class_hash).unwrap();
    }

    fn fund(address: StarkFelt, mut state: &mut State) {
        state
            .set_storage_at(
                *ETH_FEE_TOKEN_ADDRESS,
                get_storage_var_address("ERC20_balances", &[address]),
                StarkFelt::from(u128::MAX),
            )
            .unwrap_or_else(|_| panic!("failed to fund account {}", address));
    }

    macro_rules! sequencer_test {
        ($cairo_version: path, $test_name: ident) => {
            #[test]
            fn $test_name() {
                // Given
                let mut state = State::default();
                let mutable = &mut state;

                declare_and_deploy_contract(
                    &format!(
                        "src/test_data/{}/compiled_classes/counter.json",
                        $cairo_version
                    ),
                    *TEST_CONTRACT,
                    *ONE_CLASS_HASH,
                    mutable,
                    $cairo_version,
                );
                declare_and_deploy_contract(
                    &format!(
                        "src/test_data/{}/compiled_classes/account.json",
                        $cairo_version
                    ),
                    *TEST_ACCOUNT,
                    *TWO_CLASS_HASH,
                    mutable,
                    $cairo_version,
                );
                fund(*TEST_ACCOUNT.0.key(), mutable);

                let context = block_context();
                let mut sequencer = Sequencer::new(context, state, 0);

                // When
                let transaction = test_transaction();
                sequencer.execute(transaction).unwrap();

                // Then
                let expected = StarkFelt::from(1u8);
                let actual = (&mut sequencer.state)
                    .get_storage_at(*TEST_CONTRACT, get_storage_var_address("counter", &[]))
                    .unwrap();
                assert_eq!(expected, actual);
            }
        };
    }

    fn block_context() -> BlockContext {
        let block_info = BlockInfo {
            block_number: *ONE_BLOCK_NUMBER,
            block_timestamp: *ONE_BLOCK_TIMESTAMP,
            sequencer_address: *SEQUENCER_ADDRESS,
            gas_prices: GasPrices {
                eth_l1_gas_price: NonZeroU128::new(1).unwrap(),
                strk_l1_gas_price: NonZeroU128::new(1).unwrap(),
                eth_l1_data_gas_price: NonZeroU128::new(1).unwrap(),
                strk_l1_data_gas_price: NonZeroU128::new(1).unwrap(),
            },
            use_kzg_da: false,
        };

        let chain_info = ChainInfo {
            chain_id: ChainId("KKRT".into()),
            fee_token_addresses: FeeTokenAddresses {
                strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
                eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
            },
        };

        let versioned_constants = VersionedConstants::latest_constants().clone();

        let bouncer_config = Default::default();

        let concurrency_mode = Default::default();

        BlockContext::new(
            block_info,
            chain_info,
            versioned_constants,
            bouncer_config,
            concurrency_mode,
        )

        // BlockContext {
        //     block_info: BlockInfo {
        //         block_number: *ONE_BLOCK_NUMBER,
        //         block_timestamp: *ONE_BLOCK_TIMESTAMP,
        //         sequencer_address: *SEQUENCER_ADDRESS,
        //         vm_resource_fee_cost: vm_resource_fee_cost(),
        //         gas_prices: GasPrices {
        //             eth_l1_gas_price: 1,
        //             strk_l1_gas_price: 1,
        //             eth_l1_data_gas_price: 1,
        //             strk_l1_data_gas_price: 1,
        //         },
        //         use_kzg_da: false,

        //         invoke_tx_max_n_steps: 4_000_000,
        //         validate_max_n_steps: 4_000_000,
        //         max_recursion_depth: 1_000,
        //     },
        //     chain_info: ChainInfo {
        //         chain_id: ChainId("KKRT".into()),
        //         fee_token_addresses: FeeTokenAddresses {
        //             strk_fee_token_address: *STRK_FEE_TOKEN_ADDRESS,
        //             eth_fee_token_address: *ETH_FEE_TOKEN_ADDRESS,
        //         },
        //     },
        // }
    }

    // /// Maps builtins and steps to a single cost unit of reference (gas).
    // fn vm_resource_fee_cost() -> Arc<HashMap<String, f64>> {
    //     Arc::new(
    //         [
    //             (String::from("n_steps"), 1_f64),
    //             ("pedersen_builtin".to_string(), 1_f64),
    //             ("range_check_builtin".to_string(), 1_f64),
    //             ("ecdsa_builtin".to_string(), 1_f64),
    //             ("bitwise_builtin".to_string(), 1_f64),
    //             ("poseidon_builtin".to_string(), 1_f64),
    //             ("output_builtin".to_string(), 1_f64),
    //             ("ec_op_builtin".to_string(), 1_f64),
    //             ("keccak_builtin".to_string(), 1_f64),
    //             ("segment_arena_builtin".to_string(), 1_f64),
    //         ]
    //         .into_iter()
    //         .collect(),
    //     )
    // }

    fn test_transaction() -> Transaction {
        Transaction::AccountTransaction(AccountTransaction::Invoke(BlockifierInvokeTransaction {
            tx: InvokeTransaction::V1(InvokeTransactionV1 {
                sender_address: *TEST_ACCOUNT,
                calldata: Calldata(
                    vec![
                        *TEST_CONTRACT.0.key(), // destination
                        selector!("inc").into(),
                        *ZERO_FELT, // no data
                    ]
                    .into(),
                ),
                max_fee: Fee(1_000_000),
                signature: TransactionSignature(vec![]),
                nonce: Nonce(*ZERO_FELT),
            }),
            only_query: false,
            tx_hash: TransactionHash(*ZERO_FELT),
        }))
    }

    sequencer_test!(CairoVersion::V0, test_sequencer_cairo_0);
    sequencer_test!(CairoVersion::V1, test_sequencer_cairo_1);
}

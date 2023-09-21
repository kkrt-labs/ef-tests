use crate::{commit::Committer, execution::Execution};
use blockifier::{
    block_context::BlockContext,
    state::{
        cached_state::CachedState,
        state_api::{State, StateReader},
    },
    transaction::{
        errors::TransactionExecutionError, transaction_execution::Transaction,
        transactions::ExecutableTransaction,
    },
};
use tracing::{trace, warn};

/// Sequencer is the main struct of the sequencer crate.
/// Using a trait bound for the state allows for better
/// speed, as the type of the state is known at compile time.
/// We bound S such that a mutable reference to S (&'a mut S)
/// must implement State and StateReader. The `for` keyword
/// indicates that the bound must hold for any lifetime 'a.
/// For more details, check out https://doc.rust-lang.org/nomicon/hrtb.html
pub struct Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    pub context: BlockContext,
    pub state: S,
}

impl<S> Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    /// Creates a new Sequencer instance.
    pub fn new(context: BlockContext, state: S) -> Self {
        Self { context, state }
    }
}

impl<S> Execution for Sequencer<S>
where
    for<'a> &'a mut S: State + StateReader + Committer,
{
    fn execute(&mut self, transaction: Transaction) -> Result<(), TransactionExecutionError> {
        let mut cached_state = CachedState::new(&mut self.state);
        let res = transaction.execute(&mut cached_state, &self.context, false);

        match res {
            Err(err) => {
                warn!("Transaction execution failed: {:?}", err)
            }
            Ok(execution_information) => {
                <&mut S>::commit(&mut cached_state);
                match execution_information.revert_error {
                    Some(err) => {
                        warn!("Transaction execution failed: {:?}", err)
                    }
                    None => {
                        trace!("Transaction execution succeeded {execution_information:?}")
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::sync::Arc;

    use blockifier::abi::abi_utils::get_storage_var_address;
    use blockifier::execution::contract_class::{ContractClass, ContractClassV0};
    use blockifier::state::state_api::State as BlockifierState;
    use blockifier::transaction::account_transaction::AccountTransaction;
    use starknet::core::types::FieldElement;
    use starknet_api::core::{ChainId, ClassHash, ContractAddress, Nonce};
    use starknet_api::hash::StarkFelt;
    use starknet_api::transaction::{
        Calldata, Fee, InvokeTransaction, InvokeTransactionV1, TransactionHash,
        TransactionSignature,
    };

    use crate::constants::test_constants::{
        FEE_TOKEN_ADDRESS, ONE_BLOCK_NUMBER, ONE_BLOCK_TIMESTAMP, ONE_CLASS_HASH,
        SEQUENCER_ADDRESS, TEST_ADDRESS, TEST_CONTRACT_ACCOUNT, TEST_CONTRACT_ADDRESS,
        TWO_CLASS_HASH, ZERO_FELT,
    };
    use crate::state::State;

    use super::*;

    fn read_contract_class(path: &str) -> ContractClass {
        let reader = File::open(path).unwrap();
        let contract_class: ContractClassV0 = serde_json::from_reader(reader).unwrap();

        ContractClass::V0(contract_class)
    }

    fn declare_and_deploy_contract(
        path: &str,
        address: ContractAddress,
        class_hash: ClassHash,
        mut state: &mut State,
    ) {
        let contract_class = read_contract_class(path);

        state
            .set_contract_class(&class_hash, contract_class)
            .unwrap();
        state.set_class_hash_at(address, class_hash).unwrap();
    }

    fn fund(address: StarkFelt, mut state: &mut State) {
        state.set_storage_at(
            *FEE_TOKEN_ADDRESS,
            get_storage_var_address("ERC20_balances", &[address]).unwrap(),
            StarkFelt::from(u128::MAX),
        );
    }

    #[test]
    fn test_sequencer() {
        // Given
        let mut state = State::default();
        let mutable = &mut state;

        declare_and_deploy_contract(
            "src/test_data/compiled_classes/counter.json",
            *TEST_CONTRACT_ADDRESS,
            *ONE_CLASS_HASH,
            mutable,
        );
        declare_and_deploy_contract(
            "src/test_data/compiled_classes/account.json",
            *TEST_CONTRACT_ACCOUNT,
            *TWO_CLASS_HASH,
            mutable,
        );
        fund(*TEST_ADDRESS, mutable);

        let context = BlockContext {
            chain_id: ChainId("KKRT".into()),
            block_number: *ONE_BLOCK_NUMBER,
            block_timestamp: *ONE_BLOCK_TIMESTAMP,
            sequencer_address: *SEQUENCER_ADDRESS,
            fee_token_address: *FEE_TOKEN_ADDRESS,

            vm_resource_fee_cost: Arc::new(
                [
                    (String::from("n_steps"), 1_f64),
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
                .collect(),
            ),
            gas_price: 1,
            invoke_tx_max_n_steps: 4_000_000,
            validate_max_n_steps: 4_000_000,
            max_recursion_depth: 1_000,
        };
        let mut sequencer = Sequencer::new(context, state);

        // When
        let transaction = Transaction::AccountTransaction(AccountTransaction::Invoke(
            InvokeTransaction::V1(InvokeTransactionV1 {
                transaction_hash: TransactionHash(*ZERO_FELT),
                sender_address: *TEST_CONTRACT_ACCOUNT,
                calldata: Calldata(
                    vec![
                        *TEST_ADDRESS, // destination
                        FieldElement::from_hex_be(
                            "0x3b82f69851fa1625b367ea6c116252a84257da483dcec4d4e4bc270eb5c70a7",
                        ) // selector (inc)
                        .unwrap()
                        .into(),
                        *ZERO_FELT, // no data
                    ]
                    .into(),
                ),
                max_fee: Fee(1_000_000),
                signature: TransactionSignature(vec![]),
                nonce: Nonce(*ZERO_FELT),
            }),
        ));
        sequencer.execute(transaction).unwrap();

        // Then
        let expected = StarkFelt::from(1u8);
        let actual = (&mut sequencer.state)
            .get_storage_at(
                *TEST_CONTRACT_ADDRESS,
                get_storage_var_address("counter", &[]).unwrap(),
            )
            .unwrap();
        assert_eq!(expected, actual);
    }
}

#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use cairo_vm::Felt252;
use starknet_in_rust::transaction::Address as StarknetAddress;

#[macro_export]
macro_rules! starknet_storage {
    ($storage_var: expr, $felt: expr) => {
        (
            get_storage_var_address($storage_var, &[])?,
            Felt252::from($felt),
        )
    };
    ($storage_var: expr, [$($key: expr),*], $felt: expr) => {
        {
            let args = vec![$($key),*];
            (
                get_storage_var_address($storage_var, &args)?,
                Felt252::from($felt),
            )
        }
    };
}

/// Structure representing a Kakarot account.
/// Contains a nonce, Starknet storage, account
/// type, evm address and starknet address.
#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub struct KakarotAccount {
    pub(crate) starknet_address: StarknetAddress,
    pub(crate) evm_address: Felt252,
    pub(crate) nonce: Felt252,
    pub(crate) storage: Vec<(Felt252, Felt252)>,
    pub(crate) account_type: AccountType,
}

#[derive(Debug, Default, Clone)]
pub enum AccountType {
    #[default]
    Uninitialized = 0,
    EOA = 1,
    Contract = 2,
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
pub mod kkrt_account {
    use super::{AccountType, KakarotAccount};
    use cairo_vm::felt::Felt252;
    use reth_primitives::{Address, Bytes};
    use revm_primitives::U256;
    use sequencer::state::StateResult;
    use starknet_in_rust::utils::Address as StarknetAddress;

    impl KakarotAccount {
        pub fn new(
            _evm_address: &Address,
            _code: &Bytes,
            _nonce: U256,
            _evm_storage: &[(U256, U256)],
        ) -> StateResult<Self> {
            Ok(Self {
                starknet_address: StarknetAddress::default(),
                evm_address: Felt252::default(),
                nonce: Felt252::default(),
                storage: vec![],
                account_type: AccountType::EOA,
            })
        }
    }
}

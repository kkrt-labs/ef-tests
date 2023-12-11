#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use starknet_api::{
    core::{ContractAddress, Nonce},
    hash::StarkFelt,
    state::StorageKey,
};

#[macro_export]
macro_rules! starknet_storage {
    ($storage_var: expr, $felt: expr) => {
        (
            get_storage_var_address($storage_var, &[]),
            StarkFelt::from($felt),
        )
    };
    ($storage_var: expr, [$($key: expr),*], $felt: expr) => {
        {
            let args = vec![$($key),*];
            (
                get_storage_var_address($storage_var, &args),
                StarkFelt::from($felt),
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
    pub(crate) starknet_address: ContractAddress,
    pub(crate) evm_address: StarkFelt,
    pub(crate) nonce: Nonce,
    pub(crate) storage: Vec<(StorageKey, StarkFelt)>,
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
    use reth_primitives::{Address, Bytes};
    use revm_primitives::U256;
    use starknet_api::{
        core::{ContractAddress, Nonce},
        hash::StarkFelt,
        StarknetApiError,
    };

    impl KakarotAccount {
        pub fn new(
            _evm_address: &Address,
            _code: &Bytes,
            _nonce: U256,
            _evm_storage: &[(U256, U256)],
        ) -> Result<Self, StarknetApiError> {
            Ok(Self {
                starknet_address: ContractAddress::default(),
                evm_address: StarkFelt::default(),
                nonce: Nonce::default(),
                storage: vec![],
                account_type: AccountType::EOA,
            })
        }
    }
}

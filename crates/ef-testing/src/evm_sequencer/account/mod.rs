#[cfg(feature = "v0")]
pub mod v0;
#[cfg(feature = "v1")]
pub mod v1;

use starknet_api::{
    core::{ContractAddress, Nonce},
    hash::StarkFelt,
    state::StorageKey,
};

#[allow(dead_code)]
pub struct KakarotAccount {
    pub(crate) starknet_address: ContractAddress,
    pub(crate) evm_address: StarkFelt,
    pub(crate) nonce: Nonce,
    pub(crate) storage: Vec<(StorageKey, StarkFelt)>,
    pub(crate) account_type: AccountType,
}

pub enum AccountType {
    EOA = 1,
    Contract = 2,
}

#[cfg(not(any(feature = "v0", feature = "v1")))]
pub use kkrt_account::*;
#[cfg(not(any(feature = "v0", feature = "v1")))]
mod kkrt_account {
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
            _evm_storage: Vec<(U256, U256)>,
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

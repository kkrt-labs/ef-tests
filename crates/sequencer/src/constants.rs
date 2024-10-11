#[cfg(test)]
pub mod test_constants {
    use starknet::core::types::Felt;
    use starknet_api::{
        core::{ContractAddress, PatriciaKey},
        state::StorageKey,
    };

    lazy_static::lazy_static! {
        pub static ref TEST_CONTRACT: ContractAddress = ContractAddress(*ONE_PATRICIA);
        pub static ref TEST_ACCOUNT: ContractAddress = ContractAddress(*TWO_PATRICIA);
        pub static ref TEST_STORAGE_KEY: StorageKey =  StorageKey(*ONE_PATRICIA);
        pub static ref SEQUENCER_ADDRESS: ContractAddress = Felt::from(1234u16).try_into().unwrap();
        pub static ref ETH_FEE_TOKEN_ADDRESS: ContractAddress = Felt::from(12345u16).try_into().unwrap();
        pub static ref STRK_FEE_TOKEN_ADDRESS: ContractAddress = Felt::from(123456u32).try_into().unwrap();

        pub static ref ONE_PATRICIA: PatriciaKey = (Felt::ONE).try_into().unwrap();
        pub static ref TWO_PATRICIA: PatriciaKey = (Felt::TWO).try_into().unwrap();
    }
}

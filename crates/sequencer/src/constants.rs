#[cfg(test)]
pub mod test_constants {
    use starknet::core::types::Felt;
    use starknet_api::{
        block::{BlockNumber, BlockTimestamp},
        core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey},
        state::StorageKey,
    };

    lazy_static::lazy_static! {
        pub static ref TEST_CONTRACT: ContractAddress = ContractAddress(*ONE_PATRICIA);
        pub static ref TEST_ACCOUNT: ContractAddress = ContractAddress(*TWO_PATRICIA);
        pub static ref TEST_STORAGE_KEY: StorageKey =  StorageKey(*ONE_PATRICIA);
        pub static ref TEST_NONCE: Nonce =  Nonce(*ONE_FELT);
        pub static ref SENDER_ADDRESS: Felt = Felt::from(2u8);
        pub static ref SEQUENCER_ADDRESS: ContractAddress = Felt::from(1234u16).try_into().unwrap();
        pub static ref ETH_FEE_TOKEN_ADDRESS: ContractAddress = Felt::from(12345u16).try_into().unwrap();
        pub static ref STRK_FEE_TOKEN_ADDRESS: ContractAddress = Felt::from(123456u32).try_into().unwrap();

        pub static ref ZERO_FELT: Felt = Felt::from(0u8);
        pub static ref ONE_FELT: Felt = Felt::from(1u8);
        pub static ref TWO_FELT: Felt = Felt::from(2u8);
        pub static ref ONE_PATRICIA: PatriciaKey = (*ONE_FELT).try_into().unwrap();
        pub static ref TWO_PATRICIA: PatriciaKey = (*TWO_FELT).try_into().unwrap();
        pub static ref ONE_CLASS_HASH: ClassHash = ClassHash(*ONE_FELT);
        pub static ref TWO_CLASS_HASH: ClassHash = ClassHash(*TWO_FELT);
        pub static ref ONE_COMPILED_CLASS_HASH: CompiledClassHash = CompiledClassHash(*ONE_FELT);
        pub static ref ONE_BLOCK_NUMBER: BlockNumber = BlockNumber(1);
        pub static ref ONE_BLOCK_TIMESTAMP: BlockTimestamp = BlockTimestamp(1);
    }
}

#[cfg(test)]
pub mod test_constants {
    use starknet::core::types::FieldElement;
    use starknet_api::{
        block::{BlockNumber, BlockTimestamp},
        core::{ClassHash, CompiledClassHash, ContractAddress, PatriciaKey},
        hash::StarkFelt,
    };

    lazy_static::lazy_static! {
        pub static ref TEST_CONTRACT_ADDRESS: ContractAddress = ContractAddress(*ONE_PATRICIA);
        pub static ref TEST_CONTRACT_ACCOUNT: ContractAddress = ContractAddress(*TWO_PATRICIA);
        pub static ref TEST_ADDRESS: StarkFelt = *ONE_FELT;
        pub static ref TEST_CONTRACT: StarkFelt = *TWO_FELT;
        pub static ref SENDER_ADDRESS: FieldElement = FieldElement::from(2u8);
        pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(1234u16)).unwrap());
        pub static ref FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(12345u16)).unwrap());

        pub static ref ONE_FELT: StarkFelt = StarkFelt::from(1u8);
        pub static ref TWO_FELT: StarkFelt = StarkFelt::from(2u8);
        pub static ref ONE_PATRICIA: PatriciaKey = TryInto::<PatriciaKey>::try_into(*ONE_FELT).unwrap();
        pub static ref TWO_PATRICIA: PatriciaKey = TryInto::<PatriciaKey>::try_into(*TWO_FELT).unwrap();
        pub static ref ONE_CLASS_HASH: ClassHash = ClassHash(*ONE_FELT);
        pub static ref TWO_CLASS_HASH: ClassHash = ClassHash(*TWO_FELT);
        pub static ref ONE_COMPILED_CLASS_HASH: CompiledClassHash = CompiledClassHash(*ONE_FELT);
        pub static ref ONE_BLOCK_NUMBER: BlockNumber = BlockNumber(1);
        pub static ref ONE_BLOCK_TIMESTAMP: BlockTimestamp = BlockTimestamp(1);
    }
}

use lazy_static::lazy_static;
use starknet_in_rust::Felt252;

lazy_static! {
    pub static ref EXECUTE_ENTRY_POINT_SELECTOR: Felt252 =
        Felt252::from_hex("15d40a3d6ca2ac30f4031e42be28da9b056fef9bb7357ac5e85627ee876e5ad")
            .unwrap();
}

#[cfg(test)]
pub mod test_constants {
    use super::*;
    use starknet_in_rust::{
        definitions::block_context::FeeTokenAddresses,
        transaction::{Address, ClassHash},
    };

    lazy_static! {
        pub static ref ZERO: Felt252 = Felt252::from(0u8);
        pub static ref ONE: Felt252 = Felt252::from(1u8);
        pub static ref TWO: Felt252 = Felt252::from(2u8);
        pub static ref TEST_CONTRACT: Address = Address(*ONE);
        pub static ref TEST_ACCOUNT: Address = Address(*TWO);
        pub static ref SENDER_ADDRESS: Felt252 = *TWO;
        pub static ref SEQUENCER_ADDRESS: Address = Address(Felt252::from(1234));
        pub static ref CHAIN_ID: Felt252 = Felt252::from(0x4b4b5254);
        pub static ref FEE_TOKEN_ADDRESSES: FeeTokenAddresses = FeeTokenAddresses {
            eth_fee_token_address: ETH_FEE_TOKEN_ADDRESS.clone(),
            strk_fee_token_address: STARK_FEE_TOKEN_ADDRESS.clone(),
        };
        pub static ref ETH_FEE_TOKEN_ADDRESS: Address = Address(Felt252::from(12345u16));
        pub static ref STARK_FEE_TOKEN_ADDRESS: Address = Address(Felt252::from(123456u32));
        pub static ref ONE_CLASS_HASH: ClassHash = ClassHash::from(*ONE);
        pub static ref TWO_CLASS_HASH: ClassHash = ClassHash::from(*TWO);
    }
}

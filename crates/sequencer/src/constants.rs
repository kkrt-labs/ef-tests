#[cfg(test)]
pub mod test_constants {
    use starknet_in_rust::{
        felt::{felt_str, Felt252},
        utils::{Address, ClassHash},
    };

    lazy_static::lazy_static! {
        pub static ref ZERO: Felt252 = Felt252::from(0u8);
        pub static ref ONE: Felt252 = Felt252::from(1u8);
        pub static ref TWO: Felt252 = Felt252::from(2u8);

        pub static ref TEST_CONTRACT: Address = Address(ONE.clone());
        pub static ref TEST_ACCOUNT: Address = Address(TWO.clone());
        pub static ref SENDER_ADDRESS: Felt252 = TWO.clone();
        pub static ref SEQUENCER_ADDRESS: Address = Address(Felt252::from(1234));
        pub static ref CHAIN_ID: Felt252 =  Felt252::from(0x4b4b5254);
        pub static ref ETH_FEE_TOKEN_ADDRESS: Address = Address(Felt252::from(12345u16));

        pub static ref EXECUTE_ENTRY_POINT_SELECTOR: Felt252 = felt_str!(
            "617075754465154585683856897856256838130216341506379215893724690153393808813"
        );

        pub static ref ONE_CLASS_HASH: ClassHash = ClassHash::from(ONE.clone());
        pub static ref TWO_CLASS_HASH: ClassHash = ClassHash::from(TWO.clone());
    }
}

use lazy_static::lazy_static;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ClassHash, ContractAddress, PatriciaKey},
    hash::StarkFelt,
};

lazy_static! {
    // Main addresses
    pub static ref SEQUENCER_ADDRESS: ContractAddress = ContractAddress(
        TryInto::<PatriciaKey>::try_into(StarkFelt::from(
            FieldElement::from_hex_be(
                "0x01176a1bd84444c89232ec27754698e5d2e7e1a7f1539f12027f28b23ec9f3d8"
            )
            .unwrap()
        ))
        .unwrap()
    );
    pub static ref FEE_TOKEN_ADDRESS: ContractAddress = ContractAddress(
        TryInto::<PatriciaKey>::try_into(StarkFelt::from(
            FieldElement::from_hex_be(
                "0x049D36570D4e46f48e99674bd3fcc84644DdD6b96F7C741B1562B82f9e004dC7"
            )
            .unwrap()
        ))
        .unwrap()
    );
    pub static ref KAKAROT_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(1u8)).unwrap());
    pub static ref KAKAROT_OWNER_ADDRESS: ContractAddress =
        ContractAddress(TryInto::<PatriciaKey>::try_into(StarkFelt::from(2u8)).unwrap());

    // Main class hashes
    pub static ref KAKAROT_CLASS_HASH: ClassHash = ClassHash(StarkFelt::from(1u8));
    pub static ref PROXY_CLASS_HASH: ClassHash = ClassHash(StarkFelt::from(2u8));
    pub static ref CONTRACT_ACCOUNT_CLASS_HASH: ClassHash = ClassHash(StarkFelt::from(3u8));
    pub static ref EOA_CLASS_HASH: ClassHash = ClassHash(StarkFelt::from(4u8));
}

use blockifier::abi::abi_utils::get_storage_var_address as blockifier_get_storage_var_address;
use reth_primitives::{Bytes, TransactionSigned};
use reth_rlp::Decodable;
use revm_primitives::U256;
use starknet::{
    core::types::{BroadcastedInvokeTransaction, FieldElement},
    macros::selector,
};
use starknet_api::{
    core::{ClassHash, ContractAddress},
    hash::StarkFelt,
    state::StorageKey,
};

use super::{constants::KAKAROT_ADDRESS, KakarotSequencer};

pub(crate) fn get_storage_var_address(
    storage_var: &str,
    keys: &[StarkFelt],
) -> Result<StorageKey, eyre::Error> {
    Ok(blockifier_get_storage_var_address(storage_var, keys)?)
}

pub(crate) fn split_bytecode_to_starkfelt(bytecode: &Bytes) -> Vec<StarkFelt> {
    bytecode
        .chunks(16)
        .map(|x| {
            let mut storage_value = [0u8; 16];
            storage_value[..x.len()].copy_from_slice(x);
            StarkFelt::from(u128::from_be_bytes(storage_value))
        })
        .collect()
}

pub(crate) fn split_u256(value: U256) -> [u128; 2] {
    [
        (value & U256::from(u128::MAX)).try_into().unwrap(), // safe unwrap <= U128::MAX.
        (value >> 128).try_into().unwrap(),                  // safe unwrap <= U128::MAX.
    ]
}

pub(crate) fn contract_address_to_starkfelt(contract_address: &ContractAddress) -> StarkFelt {
    *contract_address.0.key()
}

pub(crate) fn class_hash_to_starkfelt(class_hash: &ClassHash) -> StarkFelt {
    class_hash.0
}

pub fn bytes_to_felt_vec(bytes: &Bytes) -> Vec<FieldElement> {
    bytes.to_vec().into_iter().map(FieldElement::from).collect()
}

#[allow(dead_code)]
pub(crate) fn to_broadcasted_starknet_transaction(
    sequencer: &KakarotSequencer,
    bytes: &Bytes,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let transaction = TransactionSigned::decode(&mut bytes.as_ref())?;

    let evm_address = transaction
        .recover_signer()
        .ok_or_else(|| eyre::eyre!("Missing signer in signed transaction"))?;

    let nonce = FieldElement::from(transaction.nonce());
    let starknet_address = sequencer.compute_starknet_address(&evm_address);

    let mut calldata = bytes_to_felt_vec(bytes);

    let mut execute_calldata: Vec<FieldElement> = vec![
        FieldElement::ONE,                                      // call array length
        contract_address_to_starkfelt(&KAKAROT_ADDRESS).into(), // contract address
        selector!("eth_send_transaction"),                      // selector
        FieldElement::ZERO,                                     // data offset
        FieldElement::from(calldata.len()),                     // data length
        FieldElement::from(calldata.len()),                     // calldata length
    ];
    execute_calldata.append(&mut calldata);

    let signature = vec![];

    let request = BroadcastedInvokeTransaction {
        max_fee: FieldElement::from(0u8),
        signature,
        nonce,
        sender_address: starknet_address.into(),
        calldata: execute_calldata,
        is_query: false,
    };

    Ok(request)
}

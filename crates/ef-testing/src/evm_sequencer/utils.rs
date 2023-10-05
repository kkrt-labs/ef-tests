use super::{
    constants::{KAKAROT_ADDRESS, PROXY_CLASS_HASH},
    types::FeltSequencer,
};
use reth_primitives::{Address, Bytes, TransactionSigned};
use reth_rlp::Decodable;
use revm_primitives::U256;
use starknet::{
    core::{
        types::{BroadcastedInvokeTransaction, FieldElement},
        utils::get_contract_address,
    },
    macros::selector,
};
use starknet_api::hash::StarkFelt;

pub fn compute_starknet_address(evm_address: &Address) -> FeltSequencer {
    let evm_address: FeltSequencer = (*evm_address).into();
    let starknet_address = get_contract_address(
        evm_address.into(),
        PROXY_CLASS_HASH.0.into(),
        &[],
        (*KAKAROT_ADDRESS.0.key()).into(),
    );
    starknet_address.into()
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

pub fn bytes_to_felt_vec(bytes: &Bytes) -> Vec<FieldElement> {
    bytes.to_vec().into_iter().map(FieldElement::from).collect()
}

pub fn felt_to_bytes(felt: &FieldElement, len: usize) -> Bytes {
    Bytes::from(&felt.to_bytes_be()[16..len + 16])
}

#[allow(dead_code)]
pub(crate) fn to_broadcasted_starknet_transaction(
    bytes: &Bytes,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let transaction = TransactionSigned::decode(&mut bytes.as_ref())?;

    let evm_address = transaction
        .recover_signer()
        .ok_or_else(|| eyre::eyre!("Missing signer in signed transaction"))?;

    let nonce = FieldElement::from(transaction.nonce());
    let starknet_address = compute_starknet_address(&evm_address);

    let mut calldata = bytes_to_felt_vec(bytes);

    let mut execute_calldata: Vec<FieldElement> = vec![
        FieldElement::ONE,                  // call array length
        (*KAKAROT_ADDRESS.0.key()).into(),  // contract address
        selector!("eth_send_transaction"),  // selector
        FieldElement::ZERO,                 // data offset
        FieldElement::from(calldata.len()), // data length
        FieldElement::from(calldata.len()), // calldata length
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_felt_to_bytes_full() {
        // Given
        let felt = FieldElement::from_hex_be("0x1234567890abcdef1234567890abcdef").unwrap();

        // When
        let bytes = felt_to_bytes(&felt, 16);

        // Then
        let expected = Bytes::from(vec![
            0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
            0xcd, 0xef,
        ]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_felt_to_bytes_partial() {
        // Given
        let felt = FieldElement::from_hex_be("0x12345678900000000000000000000000").unwrap();

        // When
        let bytes = felt_to_bytes(&felt, 5);

        // Then
        let expected = Bytes::from(vec![0x12, 0x34, 0x56, 0x78, 0x90]);
        assert_eq!(bytes, expected);
    }
}

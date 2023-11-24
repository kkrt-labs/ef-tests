use super::constants::KAKAROT_ADDRESS;
use super::types::felt::FeltSequencer;
use bytes::BytesMut;
use reth_primitives::{Address, Bytes, TransactionSigned};
use revm_primitives::U256;
use starknet::core::{
    types::{BroadcastedInvokeTransaction, FieldElement},
    utils::get_contract_address,
};
#[cfg(any(feature = "v0", feature = "v1"))]
use starknet::macros::selector;
use starknet_api::core::ClassHash;

/// Computes the Starknet address of a contract given its EVM address.
pub fn compute_starknet_address(evm_address: &Address) -> FeltSequencer {
    let evm_address: FeltSequencer = (*evm_address).try_into().unwrap(); // infallible
    let starknet_address = get_contract_address(
        evm_address.into(),
        class_hash_for_csa().0.into(),
        &constructor_calldata_for_csa(evm_address.into()),
        (*KAKAROT_ADDRESS.0.key()).into(),
    );
    starknet_address.into()
}

fn class_hash_for_csa() -> ClassHash {
    #[cfg(feature = "v0")]
    {
        return *crate::evm_sequencer::constants::kkrt_constants_v0::PROXY_CLASS_HASH;
    }

    #[cfg(feature = "v1")]
    {
        return *crate::evm_sequencer::constants::kkrt_constants_v1::UNINITIALIZED_ACCOUNT_CLASS_HASH;
    }
    ClassHash::default()
}

#[allow(clippy::missing_const_for_fn)]
fn constructor_calldata_for_csa(_evm_address: FieldElement) -> Vec<FieldElement> {
    #[cfg(feature = "v1")]
    {
        return vec![(*KAKAROT_ADDRESS.0.key()).into(), _evm_address];
    }
    vec![]
}

/// Split a U256 into low and high u128.
pub fn split_u256(value: U256) -> [u128; 2] {
    [
        (value & U256::from(u128::MAX)).try_into().unwrap(), // safe unwrap <= U128::MAX.
        (value >> 128).try_into().unwrap(),                  // safe unwrap <= U128::MAX.
    ]
}

/// Converts a byte array to a vector of FieldElement.
pub fn bytes_to_felt_vec(bytes: &Bytes) -> Vec<FieldElement> {
    bytes.to_vec().into_iter().map(FieldElement::from).collect()
}

/// Converts the high 16 bytes of a FieldElement to a byte array.
pub fn high_16_bytes_of_felt_to_bytes(felt: &FieldElement, len: usize) -> Bytes {
    Bytes::from(&felt.to_bytes_be()[16..len + 16])
}

/// Converts an signed transaction and a signature to a Starknet-rs transaction.
pub fn to_broadcasted_starknet_transaction(
    transaction: &TransactionSigned,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let evm_address = transaction
        .recover_signer()
        .ok_or_else(|| eyre::eyre!("Missing signer in signed transaction"))?;

    let nonce = FieldElement::from(transaction.nonce());
    let starknet_address = compute_starknet_address(&evm_address);

    #[allow(unused_mut)]
    let mut bytes = BytesMut::new();
    #[cfg(feature = "v0")]
    {
        transaction.encode_enveloped(&mut bytes);
    }
    #[cfg(feature = "v1")]
    {
        transaction.transaction.encode_without_signature(&mut bytes);
    }

    let mut calldata = bytes_to_felt_vec(&bytes.to_vec().into());

    let mut execute_calldata = vec![];
    #[cfg(feature = "v0")]
    {
        execute_calldata = vec![
            FieldElement::ONE,                  // call array length
            (*KAKAROT_ADDRESS.0.key()).into(),  // contract address
            selector!("eth_send_transaction"),  // selector
            FieldElement::ZERO,                 // data offset
            FieldElement::from(calldata.len()), // data length
            FieldElement::from(calldata.len()), // calldata length
        ];
    }
    #[cfg(feature = "v1")]
    {
        execute_calldata = vec![
            FieldElement::ONE,                  // call array length
            (*KAKAROT_ADDRESS.0.key()).into(),  // contract address
            selector!("eth_send_transaction"),  // selector
            FieldElement::from(calldata.len()), // calldata length
        ];
    }
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
        let bytes = high_16_bytes_of_felt_to_bytes(&felt, 16);

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
        let bytes = high_16_bytes_of_felt_to_bytes(&felt, 5);

        // Then
        let expected = Bytes::from(vec![0x12, 0x34, 0x56, 0x78, 0x90]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_felt_to_bytes_empty() {
        // Given
        let felt = FieldElement::from_hex_be("0x12345678900000000000000000000000").unwrap();

        // When
        let bytes = high_16_bytes_of_felt_to_bytes(&felt, 0);

        // Then
        assert_eq!(bytes, Bytes::default());
    }
}

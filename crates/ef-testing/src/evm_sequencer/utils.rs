use super::types::felt::FeltSequencer;
use bytes::BytesMut;
use reth_primitives::{Address, Bytes, TransactionSigned, TxType, U256};
use starknet::core::{
    types::{BroadcastedInvokeTransaction, FieldElement},
    utils::get_contract_address,
};
#[cfg(any(feature = "v0", feature = "v1"))]
use starknet::macros::selector;

/// Computes the Starknet address of a contract given its EVM address.
pub fn compute_starknet_address(
    evm_address: &Address,
    class_hash: FieldElement,
    constructor_args: &[FieldElement],
) -> FeltSequencer {
    let evm_address: FeltSequencer = (*evm_address).try_into().unwrap(); // infallible
    let starknet_address = get_contract_address(
        evm_address.into(),
        class_hash,
        constructor_args,
        0_u32.into(),
    );
    starknet_address.into()
}

/// Split a U256 into low and high u128.
pub fn split_u256(value: U256) -> [u128; 2] {
    [
        (value & U256::from(u128::MAX)).try_into().unwrap(), // safe unwrap <= U128::MAX.
        (value >> U256::from(128)).try_into().unwrap(),      // safe unwrap <= U128::MAX.
    ]
}

/// Converts a FieldElement to a byte array.
pub fn felt_to_bytes(felt: &FieldElement, start: usize) -> Bytes {
    felt.to_bytes_be()[start..].to_vec().into()
}

/// Converts an signed transaction and a signature to a Starknet-rs transaction.
pub fn to_broadcasted_starknet_transaction(
    transaction: &TransactionSigned,
    signer_starknet_address: FieldElement,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let nonce = FieldElement::from(transaction.nonce());

    let mut bytes = BytesMut::new();
    transaction.transaction.encode_without_signature(&mut bytes);

    let mut calldata: Vec<_> = bytes.into_iter().map(FieldElement::from).collect();

    let mut execute_calldata = {
        #[cfg(feature = "v0")]
        {
            use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
            vec![
                FieldElement::ONE,                 // call array length
                (*KAKAROT_ADDRESS.0.key()).into(), // contract address
                selector!("eth_send_transaction"), // selector
                FieldElement::ZERO,                // data offset
                calldata.len().into(),             // data length
                calldata.len().into(),             // calldata length
            ]
        }
        #[cfg(feature = "v1")]
        {
            use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
            vec![
                FieldElement::ONE,                 // call array length
                (*KAKAROT_ADDRESS.0.key()).into(), // contract address
                selector!("eth_send_transaction"), // selector
                calldata.len().into(),             // calldata length
            ]
        }
        #[cfg(not(any(feature = "v0", feature = "v1")))]
        {
            vec![]
        }
    };
    execute_calldata.append(&mut calldata);

    let signature = transaction.signature();
    let [r_low, r_high] = split_u256(signature.r);
    let [s_low, s_high] = split_u256(signature.s);
    let v = match transaction.transaction.tx_type() {
        TxType::Legacy => signature.v(transaction.chain_id()),
        _ => signature.odd_y_parity as u64,
    };
    let signature = vec![
        r_low.into(),
        r_high.into(),
        s_low.into(),
        s_high.into(),
        v.into(),
        FieldElement::ZERO,
    ];

    let request = BroadcastedInvokeTransaction {
        max_fee: FieldElement::ZERO,
        signature,
        nonce,
        sender_address: signer_starknet_address,
        calldata: execute_calldata,
        is_query: false,
    };

    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_felt_to_bytes {
        ($input: expr, $output: expr, $start: expr, $test_name: ident) => {
            #[test]
            fn $test_name() {
                // Given
                let felt = FieldElement::from_hex_be($input).unwrap();

                // When
                let bytes = felt_to_bytes(&felt, $start);

                // Then
                let expected = Bytes::from($output);
                assert_eq!(bytes, expected);
            }
        };
    }

    test_felt_to_bytes!(
        "0x34567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
        vec![
            0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd,
            0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90,
            0xab, 0xcd, 0xef
        ],
        1,
        test_felt_to_bytes_full
    );

    test_felt_to_bytes!(
        "0x00000000000000000000000000000000000000000000000000001234567890",
        vec![0x12, 0x34, 0x56, 0x78, 0x90],
        27,
        test_felt_to_bytes_partial
    );

    test_felt_to_bytes!(
        "0x12345678900000000000000000000000",
        vec![],
        32,
        test_felt_to_bytes_empty
    );
}

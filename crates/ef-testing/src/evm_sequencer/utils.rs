use super::constants::KAKAROT_ADDRESS;

use super::constants::{RELAYER_ADDRESS, RELAYER_SIGNING_KEY};
use super::types::felt::FeltSequencer;
use bytes::BytesMut;
use reth_primitives::{Address, Bytes, TransactionSigned, TxType, U256};
use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::{
    types::{BroadcastedInvokeTransaction, BroadcastedInvokeTransactionV1, Felt},
    utils::get_contract_address,
};

#[cfg(any(feature = "v0", feature = "v1"))]
use starknet::macros::selector;

/// Computes the Starknet address of a contract given its EVM address.
pub fn compute_starknet_address(
    evm_address: &Address,
    class_hash: Felt,
    constructor_args: &[Felt],
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

/// Converts a Felt to a byte array.
pub fn felt_to_bytes(felt: &Felt, start: usize) -> Bytes {
    felt.to_bytes_be()[start..].to_vec().into()
}

/// Converts an signed transaction and a signature to a Starknet-rs transaction.
pub fn to_broadcasted_starknet_transaction(
    transaction: &TransactionSigned,
    starknet_address: Felt,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let mut bytes = BytesMut::new();
    transaction.transaction.encode_without_signature(&mut bytes);

    let mut calldata: Vec<Felt> = {
        // Pack the calldata in 31-byte chunks.
        #[cfg(feature = "v0")]
        {
            use crate::evm_sequencer::account::pack_byte_array_to_starkfelt_array;
            std::iter::once((bytes.len()).into())
                .chain(pack_byte_array_to_starkfelt_array(
                    &bytes.into_iter().collect::<Vec<u8>>(),
                ))
                .collect()
        }
        #[cfg(not(feature = "v0"))]
        {
            bytes.into_iter().map(Felt::from).collect()
        }
    };

    // Add signature and signature length at the end of the calldata
    let signature = transaction.signature();
    let [r_low, r_high] = split_u256(signature.r);
    let [s_low, s_high] = split_u256(signature.s);
    let v = match transaction.transaction.tx_type() {
        TxType::Legacy => signature.v(transaction.chain_id()),
        _ => signature.odd_y_parity as u64,
    };
    let mut signature: Vec<Felt> = vec![
        r_low.into(),
        r_high.into(),
        s_low.into(),
        s_high.into(),
        v.into(),
    ];

    let mut execute_from_outside_calldata: Vec<Felt> = {
        #[cfg(feature = "v0")]
        {
            vec![
                (*RELAYER_ADDRESS.0.key()).into(), // caller -- OutsideExecution
                Felt::ZERO,                        // nonce (not used)
                Felt::ZERO,                        // execute after (not used in EF test)
                Felt::from(100_000_000u128),       // execute_before(not used in EF test) --
                Felt::ONE,                         // call array length
                (*KAKAROT_ADDRESS.0.key()).into(), // evm contract address --- CallArray (not used in execute_from_outside)
                selector!("eth_send_transaction"), // selector (not used in execute_from_outside)
                Felt::ZERO,                        // data offset
                calldata.len().into(),             // data length ---
                calldata.len().into(),             // calldata length
            ]
        }
        #[cfg(not(feature = "v0"))]
        {
            vec![]
        }
    };

    execute_from_outside_calldata.append(&mut calldata);
    execute_from_outside_calldata.push(signature.len().into());
    execute_from_outside_calldata.append(&mut signature);

    let mut execute_calldata = {
        #[cfg(feature = "v0")]
        {
            vec![
                Felt::ONE,                                    // CallArray len
                starknet_address, // equivalent evm contract address --- CallArray
                selector!("execute_from_outside"), // selector
                Felt::ZERO,       // data offset
                (execute_from_outside_calldata.len()).into(), // data length ---
                (execute_from_outside_calldata.len()).into(), // calldata length
            ]
        }
        #[cfg(feature = "v1")]
        {
            use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
            vec![
                Felt::ONE,                         // call array length
                *KAKAROT_ADDRESS.0.key(),          // contract address
                selector!("eth_send_transaction"), // selector
                calldata.len().into(),             // calldata length
            ]
        }
        #[cfg(not(any(feature = "v0", feature = "v1")))]
        {
            vec![]
        }
    };
    execute_calldata.append(&mut execute_from_outside_calldata);

    let data_to_hash: Vec<Felt> = vec![
        Felt::from_bytes_be_slice(b"invoke"), // invoke
        Felt::ONE,                            // version
        (*RELAYER_ADDRESS.0.key()).into(),
        Felt::ZERO,
        compute_hash_on_elements(&execute_calldata.clone()), // h(calldata)
        Felt::ZERO,                                          // max fee
        transaction.chain_id().unwrap().into(),              // chain id
        nonce,
    ];

    // Compute the hash on elements and sign it
    let transaction_hash = compute_hash_on_elements(&data_to_hash);
    let signature_starknet = RELAYER_SIGNING_KEY
        .sign(&transaction_hash)
        .expect("Signature starknet failed");

    let signature_starknet: Vec<Felt> = vec![signature_starknet.r, signature_starknet.s];

    let request = BroadcastedInvokeTransaction::V1(BroadcastedInvokeTransactionV1 {
        max_fee: Felt::ZERO,
        signature: signature_starknet,
        nonce: transaction.nonce().into(),
        sender_address: (*RELAYER_ADDRESS.0.key()).into(),
        calldata: execute_calldata,
        is_query: false,
    });

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
                let felt = Felt::from_hex($input).unwrap();

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

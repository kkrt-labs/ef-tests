use super::types::felt::FeltSequencer;
use bytes::BytesMut;
use reth_primitives::{Address, Bytes, TransactionSigned, TxType, U256};
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
#[allow(unused_variables)] // necessary for starknet_address which is behind a flag
pub fn to_broadcasted_starknet_transaction(
    transaction: &TransactionSigned,
    starknet_address: Felt,
    relayer_nonce: Option<Felt>,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let mut bytes = BytesMut::new();
    transaction.transaction.encode_without_signature(&mut bytes);

    #[allow(unused_mut)]
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

    let signature = transaction.signature();
    let [r_low, r_high] = split_u256(signature.r);
    let [s_low, s_high] = split_u256(signature.s);
    let v = match transaction.transaction.tx_type() {
        TxType::Legacy => signature.v(transaction.chain_id()),
        _ => signature.odd_y_parity as u64,
    };

    #[allow(unused_mut)]
    let mut signature: Vec<Felt> = vec![
        r_low.into(),
        r_high.into(),
        s_low.into(),
        s_high.into(),
        v.into(),
    ];

    let execute_calldata = {
        #[cfg(feature = "v0")]
        {
            use super::constants::RELAYER_ADDRESS;
            use crate::evm_sequencer::constants::KAKAROT_ADDRESS;

            let mut execute_from_outside_calldata = vec![
                *RELAYER_ADDRESS.0.key(),           // OutsideExecution caller
                Felt::ZERO,                         // OutsideExecution nonce
                Felt::ZERO,                         // OutsideExecution execute_after
                Felt::from(10_000_000_000_000u128), // OutsideExecution execute_before
                Felt::ONE,                          // call_array_len
                *KAKAROT_ADDRESS.0.key(),           // CallArray to
                selector!("eth_send_transaction"),  // CallArray selector
                Felt::ZERO,                         // CallArray data_offset
                calldata.len().into(),              // CallArray data_len
                calldata.len().into(),              // calldata_len
            ];
            execute_from_outside_calldata.append(&mut calldata);
            execute_from_outside_calldata.push(signature.len().into());
            execute_from_outside_calldata.append(&mut signature);

            let mut execute_entrypoint_calldata = vec![
                Felt::ONE,                                    // call_array_len
                starknet_address,                             // CallArray to
                selector!("execute_from_outside"),            // CallArray selector
                Felt::ZERO,                                   // CallArray data_offset
                (execute_from_outside_calldata.len()).into(), // CallArraydata data_len
                (execute_from_outside_calldata.len()).into(), // calldata length
            ];
            execute_entrypoint_calldata.append(&mut execute_from_outside_calldata);
            execute_entrypoint_calldata
        }
        #[cfg(feature = "v1")]
        {
            use crate::evm_sequencer::constants::KAKAROT_ADDRESS;
            vec![
                Felt::ONE,                         // call_array_len
                *KAKAROT_ADDRESS.0.key(),          // CallArray to
                selector!("eth_send_transaction"), // CallArray selector
                calldata.len().into(),             // CallArray data_len
            ]
        }
        #[cfg(not(any(feature = "v0", feature = "v1")))]
        {
            vec![]
        }
    };

    let request = {
        #[cfg(feature = "v0")]
        {
            use super::constants::{RELAYER_ADDRESS, RELAYER_SIGNING_KEY};
            use starknet::core::crypto::compute_hash_on_elements;

            let relayer_address = *RELAYER_ADDRESS.0.key();
            let relayer_nonce = relayer_nonce.expect("Relayer nonce not provided");
            let invoke_v1_tx = vec![
                Felt::from_bytes_be_slice(b"invoke"),        // "invoke"
                Felt::ONE,                                   // version
                relayer_address,                             // sender_address
                Felt::ZERO,                                  // 0
                compute_hash_on_elements(&execute_calldata), // h(calldata)
                Felt::ZERO,                                  // max_fee
                transaction.chain_id().unwrap().into(),      // chain_id
                relayer_nonce,                               // nonce
            ];
            let transaction_hash = compute_hash_on_elements(&invoke_v1_tx);
            let signature_relayer = RELAYER_SIGNING_KEY
                .sign(&transaction_hash)
                .expect("Signature starknet failed");
            let signature_relayer = vec![signature_relayer.r, signature_relayer.s];

            BroadcastedInvokeTransaction::V1(BroadcastedInvokeTransactionV1 {
                max_fee: Felt::ZERO,
                signature: signature_relayer,
                nonce: relayer_nonce,
                sender_address: relayer_address,
                calldata: execute_calldata,
                is_query: false,
            })
        }
        #[cfg(not(feature = "v0"))]
        {
            let nonce = Felt::from(transaction.nonce());
            BroadcastedInvokeTransaction::V1(BroadcastedInvokeTransactionV1 {
                max_fee: Felt::ZERO,
                signature,
                nonce,
                sender_address: starknet_address,
                calldata: execute_calldata,
                is_query: false,
            })
        }
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

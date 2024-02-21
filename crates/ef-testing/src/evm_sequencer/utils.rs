use super::constants::{CHAIN_ID, KAKAROT_ADDRESS};
use bytes::BytesMut;
use cairo_vm::Felt252;
use reth_primitives::{Address, Bytes, TransactionSigned, TxType};
use revm_primitives::U256;
use sequencer::constants::EXECUTE_ENTRY_POINT_SELECTOR;
use starknet::core::{types::FieldElement, utils::get_contract_address};
#[cfg(any(feature = "v0", feature = "v1"))]
use starknet::macros::selector;
use starknet_in_rust::transaction::VersionSpecificAccountTxFields;
use starknet_in_rust::utils::felt_to_field_element;
use starknet_in_rust::{
    transaction::{Address as StarknetAddress, ClassHash, InvokeFunction, Transaction},
    utils::field_element_to_felt,
};

/// Computes the Starknet address of a contract given its EVM address.
///
/// # Panics
///
/// The function will panic if the Kakarot address does not fit into a FieldElement.
pub fn compute_starknet_address(evm_address: &Address) -> StarknetAddress {
    let starknet_address = get_contract_address(
        address_to_field_element(evm_address),
        class_hash_to_field_element(&default_account_class_hash()),
        &account_constructor_args(address_to_field_element(evm_address)),
        felt_to_field_element(&KAKAROT_ADDRESS.0).unwrap(),
    );
    StarknetAddress(field_element_to_felt(&starknet_address))
}

fn default_account_class_hash() -> ClassHash {
    #[cfg(feature = "v0")]
    {
        *crate::evm_sequencer::constants::kkrt_constants_v0::PROXY_CLASS_HASH
    }

    #[cfg(feature = "v1")]
    {
        *crate::evm_sequencer::constants::kkrt_constants_v1::UNINITIALIZED_ACCOUNT_CLASS_HASH
    }
    #[cfg(not(any(feature = "v0", feature = "v1")))]
    ClassHash::default()
}

#[allow(clippy::missing_const_for_fn)]
fn account_constructor_args(_evm_address: FieldElement) -> Vec<FieldElement> {
    #[cfg(feature = "v1")]
    {
        vec![
            felt_to_field_element(&KAKAROT_ADDRESS.0).unwrap(),
            _evm_address,
        ]
    }
    #[cfg(not(feature = "v1"))]
    {
        vec![]
    }
}

/// Split a U256 into low and high u128.
pub fn split_u256(value: U256) -> [u128; 2] {
    [
        (value & U256::from(u128::MAX)).try_into().unwrap(), // safe unwrap <= U128::MAX.
        (value >> U256::from(128)).try_into().unwrap(),      // safe unwrap <= U128::MAX.
    ]
}

/// Converts the high 16 bytes of a FieldElement to a byte array.
pub fn high_16_bytes_of_felt_to_bytes(felt: &Felt252, len: usize) -> Bytes {
    Bytes::from(&felt.to_bytes_be()[16..len + 16])
}

/// Converts an signed transaction and a signature to a Starknet-rs transaction.
pub fn to_starknet_transaction(
    transaction: &TransactionSigned,
) -> Result<Transaction, eyre::Error> {
    let evm_address = transaction
        .recover_signer()
        .ok_or_else(|| eyre::eyre!("Missing signer in signed transaction"))?;

    let nonce = Felt252::from(transaction.nonce());
    let starknet_address = compute_starknet_address(&evm_address);

    let mut bytes = BytesMut::new();
    transaction.transaction.encode_without_signature(&mut bytes);

    let mut calldata: Vec<_> = bytes.into_iter().map(Felt252::from).collect();

    let mut execute_calldata = {
        #[cfg(feature = "v0")]
        {
            vec![
                Felt252::ONE,                                              // call array length
                KAKAROT_ADDRESS.0,                                         // contract address
                field_element_to_felt(&selector!("eth_send_transaction")), // selector
                Felt252::ZERO,                                             // data offset
                Felt252::from(calldata.len()),                             // data length
                Felt252::from(calldata.len()),                             // calldata length
            ]
        }
        #[cfg(feature = "v1")]
        {
            vec![
                Felt252::ONE,                                              // call array length
                KAKAROT_ADDRESS.0,                                         // contract address
                field_element_to_felt(&selector!("eth_send_transaction")), // selector
                Felt252::from(calldata.len()),                             // calldata length
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
        Felt252::from(r_low),
        Felt252::from(r_high),
        Felt252::from(s_low),
        Felt252::from(s_high),
        Felt252::from(v),
    ];

    let request = Transaction::InvokeFunction(InvokeFunction::new(
        starknet_address,
        *EXECUTE_ENTRY_POINT_SELECTOR,
        VersionSpecificAccountTxFields::Deprecated(0),
        Felt252::ONE,
        execute_calldata,
        signature,
        Felt252::from(*CHAIN_ID),
        Some(nonce),
    )?)
    // for now we ignore the fees
    .create_for_simulation(false, true, true, true, false);

    Ok(request)
}

/// Converts an EVM address to a Felt252.
pub fn address_to_felt252(address: &Address) -> Felt252 {
    Felt252::from_bytes_be_slice(address.as_bytes())
}

/// Converts an EVM address to a FieldElement. This will not panic
/// as Address is 20 bytes and FieldElement is 31 bytes.
pub fn address_to_field_element(address: &Address) -> FieldElement {
    FieldElement::from_byte_slice_be(address.as_bytes()).unwrap()
}

/// Converts a contract class hash to a FieldElement.
///
/// # Panics
///
/// This can panic if the class hash is bigger than 2^251 + 17 * 2^192.
pub fn class_hash_to_field_element(class_hash: &ClassHash) -> FieldElement {
    FieldElement::from_byte_slice_be(class_hash.to_bytes_be())
        .expect("Failed to convert class hash to FieldElement")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_felt_to_bytes_full() {
        // Given
        let felt = Felt252::from_hex("1234567890abcdef1234567890abcdef").unwrap();

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
        let felt = Felt252::from_hex("12345678900000000000000000000000").unwrap();

        // When
        let bytes = high_16_bytes_of_felt_to_bytes(&felt, 5);

        // Then
        let expected = Bytes::from(vec![0x12, 0x34, 0x56, 0x78, 0x90]);
        assert_eq!(bytes, expected);
    }

    #[test]
    fn test_felt_to_bytes_empty() {
        // Given
        let felt = Felt252::from_hex("12345678900000000000000000000000").unwrap();

        // When
        let bytes = high_16_bytes_of_felt_to_bytes(&felt, 0);

        // Then
        assert_eq!(bytes, Bytes::default());
    }
}

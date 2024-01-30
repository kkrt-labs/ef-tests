use super::constants::KAKAROT_ADDRESS;
use super::types::felt::FeltSequencer;
use bytes::BytesMut;
use reth_primitives::{Address, TransactionSigned, TxType};
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
        default_account_class_hash().0.into(),
        &account_constructor_args(evm_address.into()),
        (*KAKAROT_ADDRESS.0.key()).into(),
    );
    starknet_address.into()
}

fn default_account_class_hash() -> ClassHash {
    #[cfg(feature = "v0")]
    {
        return *crate::evm_sequencer::constants::kkrt_constants_v0::PROXY_CLASS_HASH;
    }

    #[cfg(feature = "v1")]
    {
        return *crate::evm_sequencer::constants::kkrt_constants_v1::UNINITIALIZED_ACCOUNT_CLASS_HASH;
    }
    #[cfg(not(any(feature = "v0", feature = "v1")))]
    ClassHash::default()
}

#[allow(clippy::missing_const_for_fn)]
fn account_constructor_args(_evm_address: FieldElement) -> Vec<FieldElement> {
    #[cfg(feature = "v1")]
    {
        return vec![(*KAKAROT_ADDRESS.0.key()).into(), _evm_address];
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

/// Converts an signed transaction and a signature to a Starknet-rs transaction.
pub fn to_broadcasted_starknet_transaction(
    transaction: &TransactionSigned,
) -> Result<BroadcastedInvokeTransaction, eyre::Error> {
    let evm_address = transaction
        .recover_signer()
        .ok_or_else(|| eyre::eyre!("Missing signer in signed transaction"))?;

    let nonce = FieldElement::from(transaction.nonce());
    let starknet_address = compute_starknet_address(&evm_address);

    let mut bytes = BytesMut::new();
    transaction.transaction.encode_without_signature(&mut bytes);

    let mut calldata: Vec<_> = bytes.into_iter().map(FieldElement::from).collect();

    let mut execute_calldata = {
        #[cfg(feature = "v0")]
        {
            vec![
                FieldElement::ONE,                  // call array length
                (*KAKAROT_ADDRESS.0.key()).into(),  // contract address
                selector!("eth_send_transaction"),  // selector
                FieldElement::ZERO,                 // data offset
                FieldElement::from(calldata.len()), // data length
                FieldElement::from(calldata.len()), // calldata length
            ]
        }
        #[cfg(feature = "v1")]
        {
            vec![
                FieldElement::ONE,                  // call array length
                (*KAKAROT_ADDRESS.0.key()).into(),  // contract address
                selector!("eth_send_transaction"),  // selector
                FieldElement::from(calldata.len()), // calldata length
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
        FieldElement::from(r_low),
        FieldElement::from(r_high),
        FieldElement::from(s_low),
        FieldElement::from(s_high),
        FieldElement::from(v),
    ];

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

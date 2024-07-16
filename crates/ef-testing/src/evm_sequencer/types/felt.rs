use std::convert::Infallible;

use reth_primitives::Address;
use starknet::core::types::Felt;
use starknet_api::{
    core::{ContractAddress, PatriciaKey},
    StarknetApiError,
};

/// A wrapper around a Felt in order to facilitate conversion.
#[derive(Debug, Clone, Copy)]
pub struct FeltSequencer(Felt);

impl From<Felt> for FeltSequencer {
    fn from(felt: Felt) -> Self {
        Self(felt)
    }
}

impl From<FeltSequencer> for Felt {
    fn from(felt: FeltSequencer) -> Self {
        felt.0
    }
}

impl TryFrom<Address> for FeltSequencer {
    type Error = Infallible;

    fn try_from(address: Address) -> Result<Self, Self::Error> {
        // safe unwrap since Address is 20 bytes
        Ok(Self(Felt::from_bytes_be_slice(&address.0[..])))
    }
}

impl TryFrom<FeltSequencer> for ContractAddress {
    type Error = StarknetApiError;

    fn try_from(felt: FeltSequencer) -> Result<Self, Self::Error> {
        let felt: Felt = felt.into();
        Ok(Self(TryInto::<PatriciaKey>::try_into(felt)?))
    }
}

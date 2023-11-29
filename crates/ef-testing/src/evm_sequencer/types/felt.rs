use std::convert::Infallible;

use reth_primitives::Address;
use starknet::core::types::FieldElement;
use starknet_api::{
    core::{ContractAddress, PatriciaKey},
    hash::StarkFelt,
    StarknetApiError,
};

/// A wrapper around a FieldElement in order to facilitate conversion.
#[derive(Debug, Clone, Copy)]
pub struct FeltSequencer(FieldElement);

impl From<FieldElement> for FeltSequencer {
    fn from(felt: FieldElement) -> Self {
        Self(felt)
    }
}

impl From<FeltSequencer> for FieldElement {
    fn from(felt: FeltSequencer) -> Self {
        felt.0
    }
}

impl TryFrom<Address> for FeltSequencer {
    type Error = Infallible;

    fn try_from(address: Address) -> Result<Self, Self::Error> {
        let address = FieldElement::from_byte_slice_be(&address.0[..]).unwrap(); // safe unwrap since Address is 20 bytes
        Ok(Self(address))
    }
}

impl From<FeltSequencer> for StarkFelt {
    fn from(felt: FeltSequencer) -> Self {
        Self::from(felt.0)
    }
}

impl TryFrom<FeltSequencer> for ContractAddress {
    type Error = StarknetApiError;

    fn try_from(felt: FeltSequencer) -> Result<Self, Self::Error> {
        let felt: StarkFelt = felt.into();
        let contract_address = Self(TryInto::<PatriciaKey>::try_into(felt)?);
        Ok(contract_address)
    }
}

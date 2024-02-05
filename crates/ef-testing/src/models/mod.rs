pub mod case;
pub mod error;
pub mod result;

use std::str::FromStr;

use reth_primitives::{Bytes, U64,  B256, U256, Address};
use serde::{self, de, Deserialize, Deserializer};

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct BlockchainTestTransaction {
    pub transaction: Transaction,
}

fn deserialize_address_optional<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match s.as_str() {
        "" => Ok(None),
        _ => Address::from_str(&s).map(Some).map_err(de::Error::custom), // Convert string to Address or return an error
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub data: Vec<Bytes>,
    pub gas_limit: Vec<U64>,
    pub gas_price: Option<U256>,
    pub nonce: U64,
    pub secret_key: B256,
    #[serde(deserialize_with = "deserialize_address_optional")]
    pub to: Option<Address>,
    pub sender: Address,
    pub value: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_deserialization() {
        let data = r#"
        {
            "data": ["0x1234"],
            "gasLimit": ["0x1"],
            "gasPrice": "0x1",
            "nonce": "0x00",
            "secretKey": "0x0000000000000000000000000000000000000000000000000000000000000123",
            "sender": "0x00000000000000000000000000000000deadbeef",
            "to": "",
            "value": ["0x01"]
        }
        "#;

        // Attempt to deserialize the JSON data
        let result: Result<Transaction, _> = serde_json::from_str(data);

        // Check if deserialization was successful
        assert!(result.is_ok(), "Failed to deserialize: {:?}", result.err());
    }
}

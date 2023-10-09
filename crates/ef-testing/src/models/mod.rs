pub mod case;
pub mod error;
pub mod result;
pub mod suite;

use std::str::FromStr;

use reth_primitives::{Bytes, U128, U64};
use revm_primitives::{B160, B256};
use serde::{self, de, Deserialize, Deserializer};

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct BlockchainTestTransaction {
    pub transaction: Transaction,
}

fn deserialize_b160_optional<'de, D>(deserializer: D) -> Result<Option<B160>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    match s.as_str() {
        "" => Ok(None),
        _ => B160::from_str(&s).map(Some).map_err(de::Error::custom), // Convert string to B160 or return an error
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transaction {
    pub data: Vec<Bytes>,
    pub gas_limit: Vec<U64>,
    pub gas_price: U64,
    pub nonce: U64,
    pub secret_key: B256,
    #[serde(deserialize_with = "deserialize_b160_optional")]
    pub to: Option<B160>,
    pub sender: B160,
    pub value: Vec<U128>,
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

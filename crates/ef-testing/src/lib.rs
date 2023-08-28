pub mod models;
pub mod storage;
pub mod traits;
pub mod utils;

use std::path::Path;

use bytes::BytesMut;
use kakarot_rpc_core::client::constants::CHAIN_ID;
use reth_primitives::{sign_message, Bytes, SealedBlock, Signature, Transaction};
use reth_rlp::Decodable;
use revm_primitives::B256;

/// Sign a transaction given a private key and a chain id.
pub fn sign_tx_with_chain_id(
    tx: &mut Transaction,
    private_key: &B256,
    chain_id: u64,
) -> Result<Signature, eyre::Error> {
    tx.set_chain_id(chain_id);
    let signature = sign_message(*private_key, tx.signature_hash())?;
    Ok(signature)
}

pub fn get_signed_rlp_encoded_transaction(
    block: &Bytes,
    pk: B256,
) -> Result<Bytes, ef_tests::Error> {
    // parse it as a sealed block
    let mut block =
        SealedBlock::decode(&mut block.as_ref()).map_err(ef_tests::Error::RlpDecodeError)?;

    // encode body as transaction
    let mut out = BytesMut::new();
    let tx_signed = block.body.get_mut(0).unwrap();

    let tx = &mut tx_signed.transaction;
    tx.set_chain_id(CHAIN_ID);
    let signature = sign_tx_with_chain_id(tx, &pk, CHAIN_ID).unwrap();
    tx_signed.encode_with_signature(&signature, &mut out, true);

    Ok(out.to_vec().into())
}

/// returns whether a given test at a given path should be skipped or not
pub fn should_skip(path: &Path) -> bool {
    let path_str = path.to_str().expect("Path is not valid UTF-8");
    let name = path.file_name().unwrap().to_str().unwrap();

    // list of files that we do want to include even though their parent directories might be marked for skip
    let include_files = [
        "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/mul.json",
    ];
    for file_path in include_files {
        if path_str.contains(file_path) {
            return false;
        }
    }

    matches!(
        name,
        // A case can be added like this
        // | "testFileName.json"
        // using "" as a placeholder till we get our first filename to ignore
        | ""
    ) || path_contains(
        path_str,
        &[
            "ABITests",
            "BasicTests",
            "BlockchainTests",
            "DifficultyTests",
            "EIPTests",
            "EOFTests",
            "GenesisTests",
            "JSONSchema",
            "KeyStoreTests",
            "LegacyTests",
            "PoWTests",
            "RLPTests",
            "TransactionTests",
            "TrieTests",
        ],
    )
}

/// returns whether a path is part from a list of directories
fn path_contains(path_str: &str, dirs: &[&str]) -> bool {
    for value in dirs {
        if path_str.contains(value) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::should_skip;

    #[tokio::test]
    async fn test_should_skip() {
        // should be skipped since BasicTests is ignored
        let path = Path::new("ethereum-tests/BasicTests/blockgenesistest.json");

        assert!(should_skip(path));

        // should not be skipped although BlockchainTests dir has been ignored, since it is included in `include_files`
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/add.json",
        );
        assert!(!should_skip(path));

        // should not be skipped although BlockchainTests dir has been ignored, since it is included in `include_files`
        let path = Path::new(
            "ethereum-tests/BlockchainTests/GeneralStateTests/VMTests/vmArithmeticTest/mul.json",
        );
        assert!(!should_skip(path));
    }
}

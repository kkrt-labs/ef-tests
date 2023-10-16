//! Procedural macros.
mod constants;
mod converter;
mod dir_reader;
mod filter;
mod utils;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::path::PathBuf;

use crate::{converter::TestConverter, dir_reader::DirReader};

#[proc_macro]
pub fn generate_blockchain_tests(_input: TokenStream) -> TokenStream {
    read_tests_to_stream().into()
}

fn read_tests_to_stream() -> TokenStream2 {
    // TODO parse the Blockchain tests, generate the following to store in the test as raw string:
    //  - private key
    //  - pre and post state
    //  - blocks
    // TODO parse the raw string into the corresponding values (B256, State, Option<RootOrState>, Vec<Block>)
    // TODO Run the test with the parsed elements
    let root_node = DirReader::new();
    let suite_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests");
    let root_node = root_node
        .walk_directory(suite_path)
        .expect("Error while walking directory");

    // First level should only contain folders
    assert!(root_node.files.is_empty());

    let converter = TestConverter::new(root_node);
    let tests = converter
        .convert()
        .expect("Error while converting the tests");

    let tests = syn::parse_str::<TokenStream2>(&tests).expect("Error while parsing the test");
    quote! {
        #tests
    }
}

#[cfg(test)]
mod blockchain_tests_trial {
    use super::*;

    #[test]
    fn test_stream_tests() {
        let _ = read_tests_to_stream().to_string();
    }
}

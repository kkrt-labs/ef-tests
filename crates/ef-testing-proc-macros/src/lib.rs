//! Procedural macros.
// mod blockchain_data_reader;
mod constants;
mod content_reader;
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
    let root_node = DirReader::new();
    let suite_path: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests");
    let root_node = root_node
        .walk_dir_and_store_files(suite_path.into())
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

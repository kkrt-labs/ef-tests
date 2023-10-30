use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use build_utils::{converter::EfTests, dir_reader::DirReader, filter::Filter};
use lazy_static::lazy_static;

const SKIPPED_TESTS: &str = "../../blockchain-tests-skip.yml";
const CACHED_SKIPPED_TESTS: &str = "../../tmp/blockchain-tests-skip.yml";

lazy_static! {
    static ref SUITE_PATH: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("./ethereum-tests/BlockchainTests/GeneralStateTests");
    static ref INTEGRATION_TESTS_PATH: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("./tests/");
}

fn main() {
    // Tell Cargo that if the given file changes -> to rerun this build script.
    println!("cargo:rerun-if-changed={SKIPPED_TESTS}");
    let filter = Filter::load_file(SKIPPED_TESTS).expect("Unable to load test filter file");
    let maybe_cached_filter = Filter::load_file(CACHED_SKIPPED_TESTS).ok();

    // Get the diff between the cached filter and the current one
    let mut target = maybe_cached_filter.map(|cached_filter| filter.diff(&cached_filter));

    // Check that we already have tests in the folder
    let mut current_tests = DirReader::walk_dir((INTEGRATION_TESTS_PATH.clone()).into());
    if current_tests.next().is_none() {
        // If not, ignore the target
        target = None;
    }

    // Walk the directory and store all files based on diff (or all files if no diff)
    let root_node = DirReader::new(&target);
    let root_node = root_node
        .walk_dir_and_store_files(SUITE_PATH.clone().into())
        .expect("Error while walking directory");

    // First level should only contain folders
    assert!(root_node.files().is_empty());

    // Convert all test files
    let filter = Arc::new(filter);
    let converter = EfTests::new(root_node, filter.clone());
    let tests = converter
        .convert()
        .expect("Error while converting the tests");

    fs::create_dir_all(INTEGRATION_TESTS_PATH.clone())
        .expect("Unable to create dir integration tests folder");
    // Write all tests to the integration tests folder
    for (folder_name, content) in tests {
        let mut path = INTEGRATION_TESTS_PATH.join(folder_name);
        path.set_extension("rs");
        fs::write(path, content).expect("Unable to write file");
    }

    // Cache the filter
    let filter_content =
        serde_yaml::to_string(filter.as_ref()).expect("Unable to serialize filter");
    let cached_skipped_tests_path = Path::new(CACHED_SKIPPED_TESTS)
        .parent()
        .expect("Unable to get parent dir");
    fs::create_dir_all(cached_skipped_tests_path).expect("Unable to create tmp folder");
    fs::write(CACHED_SKIPPED_TESTS, filter_content).expect("Unable to write file");
}

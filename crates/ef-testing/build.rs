use std::{path::PathBuf, sync::Arc, time::Instant};

use build_utils::{converter::EfTests, dir_reader::DirReader, filter::Filter};
use lazy_static::lazy_static;

const SKIPPED_TESTS: &str = "../../blockchain-tests-skip.yml";
const CACHED_SKIPPED_TESTS: &str = "../../tmp/blockchain-tests-skip.yml";

lazy_static! {
    static ref SUITE_PATH: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ef-testing/ethereum-tests/BlockchainTests/GeneralStateTests");
}

fn main() {
    // // Tell Cargo that if the given file changes, to rerun this build script.
    // let now = Instant::now();
    // println!("cargo:rerun-if-changed={SKIPPED_TESTS}");
    // let filter = Filter::load_file(SKIPPED_TESTS).expect("Unable to load test filter file");
    // let cached_filter = Filter::load_file(CACHED_SKIPPED_TESTS).ok();
    // match cached_filter {
    //     Some(cf) => {
    //         let diff = filter.diff(cf);
    //     }
    //     None => {
    //         let filter = Arc::new(filter);
    //         let root_node = DirReader::new(filter.clone());
    //         let root_node = root_node
    //             .walk_dir_and_store_files(SUITE_PATH.clone().into())
    //             .expect("Error while walking directory");

    //         // First level should only contain folders
    //         assert!(root_node.files().is_empty());

    //         let converter = EfTests::new(root_node, filter);
    //         let tests = converter
    //             .convert()
    //             .expect("Error while converting the tests");
    //     }
    // };
    // let elapsed_time = now.elapsed();
    // println!(
    //     "cargo:warning=building took {} seconds",
    //     elapsed_time.as_secs()
    // );
    // // let cached_skipped_tests = fs::read_to_string(CACHED_SKIPPED_TESTS).ok();
    // // match cached_skipped_tests
    // // let mut config_contents = String::new();
    // // config_file
    // //     .read_to_string(&mut config_contents)
    // //     .expect("Unable to read config file");
    // // // ...
}

//! Traits definition
//! Inspired by <https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests>

use async_trait::async_trait;
use std::{
    fmt::Debug,
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

use crate::models::{case::Cases, error::RunnerError, result::assert_tests_pass};

/// A single test case, capable of loading a JSON description of itself and running it.
#[async_trait]
pub trait Case: Debug + Sync + Send + Sized {
    /// A description of the test.
    fn description(&self) -> String {
        "no description".to_string()
    }

    /// Load the test from the given file path
    ///
    /// The file can be assumed to be a valid EF test case as described on <https://ethereum-tests.readthedocs.io/>.
    fn load(path: &Path) -> Result<Self, RunnerError>;

    /// Run the test on the Katana test context.
    async fn run(&self) -> Result<(), RunnerError>;
}

#[async_trait]
pub trait Suite {
    /// The type of test cases in this suite.
    type Case: Case;

    /// The name of the test suite used to locate the individual test cases.
    ///
    /// # Example
    ///
    /// - `GeneralStateTests`
    /// - `BlockchainTests/InvalidBlocks`
    /// - `BlockchainTests/TransitionTests`
    fn suite_name(&self) -> String;

    /// Load an run each contained asynchronously test case.
    async fn run(&self) {
        let suite_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ethereum-tests")
            .join(self.suite_name());

        // todo: assert that the path exists
        let test_cases_paths = find_all_files_with_extension(&suite_path, ".json");

        let mut test_cases = Vec::new();

        for test_case_path in test_cases_paths {
            let case = Self::Case::load(&test_case_path).expect("test case should load");
            test_cases.push((test_case_path, case));
        }

        let results = Cases { test_cases }.run().await;

        assert_tests_pass(&results);
    }
}

/// Recursively find all files with a given extension.
fn find_all_files_with_extension(path: &Path, extension: &str) -> Vec<PathBuf> {
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(extension))
        .map(DirEntry::into_path)
        .collect::<_>()
}

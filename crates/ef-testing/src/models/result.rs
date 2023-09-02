// Inspired by https://github.com/paradigmxyz/reth/tree/main/testing/ef-tests

use std::path::{Path, PathBuf};

use crate::traits::Case;

use super::error::RunnerError;

/// The result of running a test.
#[derive(Debug)]
pub struct CaseResult {
    /// A description of the test.
    pub desc: String,
    /// The full path to the test.
    pub path: PathBuf,
    /// The result of the test.
    pub result: Result<(), RunnerError>,
}

impl CaseResult {
    /// Create a new test result.
    pub fn new(path: &Path, case: &impl Case, result: Result<(), RunnerError>) -> Self {
        CaseResult {
            desc: case.description(),
            path: path.into(),
            result,
        }
    }
}

/// Assert that all the given tests passed and print the results to stdout.
pub(crate) fn assert_tests_pass(suite_name: &str, path: &Path, results: &[CaseResult]) {
    let (passed, failed, skipped) = categorize_results(results);

    print_results(suite_name, path, &passed, &failed, &skipped);

    if !failed.is_empty() {
        panic!("Some tests failed (see above)");
    }
}

/// Categorize test results into `(passed, failed, skipped)`.
pub(crate) fn categorize_results(
    results: &[CaseResult],
) -> (Vec<&CaseResult>, Vec<&CaseResult>, Vec<&CaseResult>) {
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut skipped = Vec::new();

    for case in results {
        match case.result.as_ref().err() {
            Some(RunnerError::Skipped) => skipped.push(case),
            Some(_) => failed.push(case),
            None => passed.push(case),
        }
    }

    (passed, failed, skipped)
}

/// Display the given test results to stdout.
pub(crate) fn print_results(
    suite_name: &str,
    path: &Path,
    passed: &[&CaseResult],
    failed: &[&CaseResult],
    skipped: &[&CaseResult],
) {
    println!("\n Suite: {suite_name} (at {})", path.display());
    println!(
        "Ran {} tests ({} passed, {} failed, {} skipped) \n",
        passed.len() + failed.len() + skipped.len(),
        passed.len(),
        failed.len(),
        skipped.len()
    );

    for case in skipped {
        println!("[S] Case {} skipped", case.path.display());
    }

    for case in failed {
        let error = case.result.clone().unwrap_err();

        println!("[!] Case {} failed:\n{}", case.path.display(), error);
    }
}

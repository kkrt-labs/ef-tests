use std::collections::BTreeMap;

use ef_tests::models::BlockchainTest;
use rayon::prelude::*;
use regex::Regex;

use crate::{
    constants::{FORK, REGEX_STR},
    dir_reader::{DirReader, PathWrapper},
};

/// The TestConverter is used to convert all the
/// tests files in to rust tests.
/// Filter is an optional filter to be applied on
/// the test file's name.
pub struct TestConverter {
    directory: DirReader,
}

impl TestConverter {
    pub fn new(directory: DirReader) -> Self {
        Self { directory }
    }

    pub fn convert(&self) -> Result<String, eyre::Error> {
        self.directory
            .sub_dirs
            .par_iter()
            .fold(
                || Ok(String::new()),
                |acc, (folder_name, node)| {
                    let mut s = String::new();
                    s += &Self::format_to_module(folder_name);
                    s += &Self::convert_folders(node)?;
                    s += "}";
                    Ok(acc? + &s)
                },
            )
            .collect()
    }

    fn convert_folders(node: &DirReader) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for (dir_name, sub_node) in &node.sub_dirs {
            acc += &TestConverter::format_to_module(dir_name);
            acc += &Self::convert_folders(sub_node)?;
            acc += "}";
        }
        Ok(acc + &Self::convert_files(&node.files)?)
    }

    fn convert_files(files: &[(PathWrapper, bool)]) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for (file_path, is_skipped) in files {
            let content = file_path.read_file_to_string()?;
            // let general_state_tests_path = path
            // .components()
            // .filter(|x| !x.as_os_str().eq_ignore_ascii_case("BlockchainTests"))
            // .collect::<PathBuf>();
            let cases: BTreeMap<String, serde_json::Value> = serde_json::from_str(&content)?;
            acc += &cases.into_keys().fold(String::new(), |acc, case_name| {
                if !case_name.contains(FORK) {
                    return acc;
                }
                acc + &TestConverter::format_to_test(&case_name, *is_skipped)
            })
        }
        Ok(acc)
    }

    fn format_to_module(folder_name: &str) -> String {
        format!(
            r#"mod {} {{"#,
            TestConverter::format_into_identifier(folder_name)
        )
    }

    fn format_to_test(case_name: &str, is_skipped: bool) -> String {
        format!(
            r#"
            #[tokio::test(flavor = "multi_thread")]
            {}
            async fn test_{}() {{
            }}"#,
            if is_skipped { "#[ignore]" } else { "" },
            TestConverter::format_into_identifier(case_name),
        )
    }

    fn format_into_identifier(s: &str) -> String {
        let re = Regex::new(REGEX_STR).expect("Error while compiling the regex"); // safe unwrap
        re.replace_all(s, "_")
            .replace('-', "_minus_")
            .replace('+', "_plus_")
    }
}

use std::collections::BTreeMap;

use rayon::prelude::*;
use serde_json::Value;

use crate::{
    constants::{FORK, UNSUPPORTED_IDENTIFIER_CHAR},
    content_reader::ContentReader,
    dir_reader::DirReader,
    path::PathWrapper,
};

/// The TestConverter is used to convert the directory structure
/// into a String containing all the rust tests to be ran.
///
/// # Example
///
/// Test location: BlockchainTests/GeneralStateTests/stRandom/
/// List of tests: [randomStatetest0.json, randomStatetest1.json, ...]
/// Inner tests: [randomStatetest0_d0g0v0_Shanghai, randomStatetest0_d1g0v0_Shanghai,
/// ..., randomStatetest1_d0g0v0_Shanghai, randomStatetest1_d1g0v0_Shanghai, ...]
/// Generated String:
/// r#"
/// mod randomStatetest0 {
///   use super::*;
///   #[test]
///   fn test_randomStatetest0_d0g0v0_Shanghai() {
///     ...
///   }
///   #[test]
///   fn test_randomStatetest0_d1g0v0_Shanghai() {
///     ...
///   }
///   #[test]
///   fn test_randomStatetest1_d0g0v0_Shanghai() {
///     ...
///   }
///   #[test]
///   fn test_randomStatetest1_d1g0v0_Shanghai() {
///     ...
///   }
///   ...
/// }
/// "#
pub struct TestConverter {
    directory: DirReader,
}

impl TestConverter {
    pub fn new(directory: DirReader) -> Self {
        Self { directory }
    }

    /// Converts the given directory into a String containing all
    /// the rust tests to be ran.
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

    /// Converts the given directory into a String.
    fn convert_folders(node: &DirReader) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for (dir_name, sub_node) in &node.sub_dirs {
            acc += &TestConverter::format_to_module(dir_name);
            acc += &Self::convert_folders(sub_node)?;
            acc += "}";
        }
        Ok(acc + &Self::convert_files(&node.files)?)
    }

    #[allow(clippy::manual_try_fold)]
    /// Converts the given files into a String.
    fn convert_files(files: &[(PathWrapper, bool)]) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for (file_path, is_skipped) in files {
            let content = file_path.read_file_to_string()?;
            let cases: BTreeMap<String, serde_json::Value> = serde_json::from_str(&content)?;
            acc += &cases.into_iter().fold(
                Ok(String::new()),
                |acc: Result<String, eyre::Error>, (case_name, content)| {
                    if !case_name.contains(FORK) {
                        return acc;
                    }
                    let secret_key = ContentReader::secret_key(file_path.clone())?;
                    Ok(acc?
                        + &TestConverter::format_to_test(
                            &case_name,
                            secret_key,
                            content,
                            *is_skipped,
                        )?)
                },
            )?
        }
        Ok(acc)
    }

    /// Formats the given folder name into a rust module.
    fn format_to_module(folder_name: &str) -> String {
        format!(
            r#"mod {} {{
            use super::*;
            "#,
            TestConverter::format_into_identifier(folder_name)
        )
    }

    /// Formats the given test case into a rust test.
    fn format_to_test(
        case_name: &str,
        secret_key: Value,
        content: Value,
        is_skipped: bool,
    ) -> Result<String, eyre::Error> {
        Ok(format!(
            r#"
            #[test]
            {}
            fn test_{}() {{
                {}
            }}"#,
            if is_skipped { "#[ignore]" } else { "" },
            TestConverter::format_into_identifier(case_name),
            Self::format_test_content(case_name, secret_key, &content, is_skipped)?,
        ))
    }

    /// Formats the given test content into a rust test.
    fn format_test_content(
        case_name: &str,
        secret_key: Value,
        content: &Value,
        is_skipped: bool,
    ) -> Result<String, eyre::Error> {
        if is_skipped {
            return Ok(String::default());
        }
        let block = ContentReader::block(content)?;
        let pre = ContentReader::pre_state(content)?;
        let post = ContentReader::post_state(content)?;
        Ok(format!(
            r##"
            setup();
            let block: Block = serde_json::from_str(r#"{}"#).expect("Error while reading the block");
            let pre: State = serde_json::from_str(r#"{}"#).expect("Error while reading the pre state");
            let post: RootOrState = serde_json::from_str(r#"{}"#).expect("Error while reading the post state");
            let case = BlockchainTestCase::new("{}".to_string(), block, pre, post, B256::from_str({}).expect("Error while reading  secret key"));
            case.run().expect("Error while running the test");
        "##,
            block, pre, post, case_name, secret_key
        ))
    }

    /// Formats the given string into a valid rust identifier.
    fn format_into_identifier(s: &str) -> String {
        UNSUPPORTED_IDENTIFIER_CHAR
            .replace_all(s, "_")
            .replace('-', "_minus_")
            .replace('+', "_plus_")
    }
}

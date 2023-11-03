use std::{collections::BTreeMap, sync::Arc};

use rayon::prelude::*;
use serde_json::Value;

use crate::{
    constants::{FORK, UNSUPPORTED_IDENTIFIER_CHAR},
    content_reader::ContentReader,
    dir_reader::DirReader,
    filter::Filter,
    path::PathWrapper,
};

/// The `TestConverter` is used to convert the directory structure
/// into a String containing all the rust tests to be ran.
///
/// # Example
///
/// Test location: BlockchainTests/GeneralStateTests/stRandom/
/// List of tests: [randomStatetest0.json, randomStatetest1.json, ...]
/// Inner tests: [`randomStatetest0_d0g0v0_Shanghai`, `randomStatetest0_d1g0v0_Shanghai`,
/// ..., `randomStatetest1_d0g0v0_Shanghai`, `randomStatetest1_d1g0v0_Shanghai`, ...]
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
pub struct EfTests<'a> {
    directory: DirReader<'a>,
    filter: Arc<Filter>,
}

impl<'a> EfTests<'a> {
    pub const fn new(directory: DirReader<'a>, filter: Arc<Filter>) -> Self {
        Self { directory, filter }
    }

    /// Converts the given directory into a String containing all
    /// the rust tests to be ran.
    pub fn convert(&self) -> Result<Vec<(String, String)>, eyre::Error> {
        self.directory
            .sub_dirs
            .iter()
            .map(|(folder_name, node)| {
                let mut acc = String::new();
                acc += &Self::format_to_folder();
                acc += &Self::format_to_module(folder_name);
                acc += &self.convert_folders(node)?;
                acc += "}";
                Ok((folder_name.clone(), acc))
            })
            .collect()
    }

    /// Converts the given directory into a String.
    fn convert_folders(&self, node: &DirReader) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for (dir_name, sub_node) in &node.sub_dirs {
            acc += &Self::format_to_module(dir_name);
            acc += &self.convert_folders(sub_node)?;
            acc += "}";
        }
        acc += &self.convert_files(&node.files)?.as_str();
        Ok(acc)
    }

    #[allow(clippy::manual_try_fold)]
    /// Converts the given files into a String.
    fn convert_files(&self, files: &[PathWrapper]) -> Result<String, eyre::Error> {
        let mut acc = String::new();
        for file_path in files {
            let content = file_path.read_file_to_string()?;
            let cases: BTreeMap<String, serde_json::Value> = serde_json::from_str(&content)?;
            let file_contents = cases
                .par_iter()
                .map(|(case_name, content)| {
                    if !case_name.contains(FORK) {
                        return Ok(String::new());
                    }
                    let secret_key = ContentReader::secret_key(file_path.clone())?
                        .ok_or_else(|| eyre::eyre!("Missing secret key"))?;
                    let is_skipped = self.filter.is_skipped(file_path, Some(case_name.clone()));
                    Self::format_to_test(case_name, &secret_key, content, is_skipped)
                })
                .collect::<Result<Vec<String>, eyre::Error>>()?;
            acc += &file_contents.into_iter().fold(String::new(), |mut acc, s| {
                acc += &s;
                acc
            });
        }
        Ok(acc)
    }

    /// Formats the given folder name into a rust module.
    fn format_to_folder() -> String {
        r"
        #![allow(warnings)]
        use std::{str::FromStr};

        use ef_testing::models::case::BlockchainTestCase;
        use ef_testing::test_utils::setup;
        use ef_testing::traits::Case;
        use ef_tests::models::{Block, RootOrState, State};
        use revm_primitives::B256;
        "
        .to_string()
    }

    /// Formats the given folder name into a rust module.
    fn format_to_module(folder_name: &str) -> String {
        format!(
            r#"mod {} {{
            use super::*;
            "#,
            Self::format_into_identifier(folder_name)
        )
    }

    /// Formats the given test case into a rust test.
    fn format_to_test(
        case_name: &str,
        secret_key: &Value,
        content: &Value,
        is_skipped: bool,
    ) -> Result<String, eyre::Error> {
        let test_content = Self::format_test_content(case_name, secret_key, content, is_skipped);
        let test_content_err = test_content.as_ref().map_err(|err| err.to_string());

        let test_header = Self::format_test_header(is_skipped, test_content_err.err());
        let test_content = test_content.unwrap_or_default();
        let test_name = Self::format_into_identifier(case_name);

        Ok(format!(
            r#"
            #[test]
            {test_header}
            fn test_{test_name}() {{
                {test_content}
            }}"#,
        ))
    }

    /// Formats the given test content into a rust test.
    fn format_test_content(
        case_name: &str,
        secret_key: &Value,
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
            let block: Block = serde_json::from_str(r#"{block}"#).expect("Error while reading the block");
            let pre: State = serde_json::from_str(r#"{pre}"#).expect("Error while reading the pre state");
            let post: RootOrState = serde_json::from_str(r#"{post}"#).expect("Error while reading the post state");
            let case = BlockchainTestCase::new("{case_name}".to_string(), block, pre, post, B256::from_str({secret_key}).expect("Error while reading  secret key"));
            case.run().expect("Error while running the test");
        "##
        ))
    }

    fn format_test_header(is_skipped: bool, content_err: Option<String>) -> String {
        if is_skipped {
            return String::from("#[ignore = \"skipped in config file\"]");
        } else if content_err.is_some() {
            return format!("#[ignore = \"{}\"]", content_err.unwrap());
        }
        String::default()
    }

    /// Formats the given string into a valid rust identifier.
    fn format_into_identifier(s: &str) -> String {
        UNSUPPORTED_IDENTIFIER_CHAR
            .replace_all(s, "_")
            .replace('-', "_minus_")
            .replace('+', "_plus_")
    }
}

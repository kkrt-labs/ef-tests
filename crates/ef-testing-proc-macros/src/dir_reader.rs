use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use walkdir::WalkDir;

use crate::constants::ROOT;
use crate::constants::SKIPPED_TESTS;
use crate::filter::Filter;
use crate::path::PathWrapper;
use crate::utils::path_relative_to;
use crate::utils::path_to_vec_string;

/// The `DirReader` will iterate all folders and
/// files in the given directory and stores them
/// by using a recursive structure (structure that
/// contains itself).
pub struct DirReader {
    /// Mapping containing the sub directories
    pub(crate) sub_dirs: BTreeMap<String, DirReader>,
    /// Vector containing the files and wether they should be skipped
    pub(crate) files: Vec<(PathWrapper, bool)>,
    /// Filter to be applied on the files
    filter: Arc<Filter>,
}

impl DirReader {
    pub fn new() -> Self {
        Self {
            sub_dirs: BTreeMap::default(),
            files: Vec::default(),
            filter: Arc::new(Filter::new(SKIPPED_TESTS)),
        }
    }

    /// Walks the given directory and stores all files
    pub fn walk_dir_and_store_files(
        mut self,
        directory_path: PathWrapper,
    ) -> Result<Self, eyre::Error> {
        for entry in WalkDir::new(Into::<PathBuf>::into(directory_path))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|f| f.file_type().is_file())
        {
            let full_path = entry.path();
            let path = path_to_vec_string(full_path)?;
            self.insert_file(path_relative_to(path, ROOT), full_path.to_path_buf().into());
        }
        Ok(self)
    }

    /// Inserts a file into the `DirReader` by recursively navigating the file's
    /// path and inserting the file into the correct sub directory.
    fn insert_file(&mut self, current_path: Vec<String>, full_path: PathWrapper) {
        if current_path.len() > 1 {
            let root_name = current_path.first().cloned().unwrap(); // safe unwrap
            let sub_node = self.sub_dirs.entry(root_name).or_insert_with(|| Self {
                sub_dirs: BTreeMap::default(),
                files: Vec::default(),
                filter: self.filter.clone(),
            });
            sub_node.insert_file(current_path.into_iter().skip(1).collect(), full_path);
        } else {
            let skip = self.filter.is_skipped(&full_path);
            self.files.push((full_path, skip));
        }
    }
}

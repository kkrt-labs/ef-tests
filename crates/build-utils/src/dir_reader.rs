use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use walkdir::DirEntry;
use walkdir::WalkDir;

use crate::constants::ROOT;
use crate::path::PathWrapper;
use crate::utils::path_relative_to;
use crate::utils::path_to_vec_string;

/// The `DirReader` will iterate all folders and
/// files in the given directory and stores them
/// by using a recursive structure (structure that
/// contains itself).
pub struct DirReader<'a> {
    /// Mapping containing the sub directories
    pub(crate) sub_dirs: BTreeMap<String, DirReader<'a>>,
    /// Vector containing the files
    pub(crate) files: Vec<PathWrapper>,
    /// List of folders that should be considered by the `DirReader`
    target: &'a Option<Vec<String>>,
}

impl<'a> DirReader<'a> {
    pub fn new(target: &'a Option<Vec<String>>) -> Self {
        Self {
            sub_dirs: BTreeMap::default(),
            files: Vec::default(),
            target,
        }
    }

    /// Returns the test files paths
    pub fn files(&self) -> &[PathWrapper] {
        &self.files
    }

    /// Walks the given directory
    pub fn walk_dir(directory_path: PathWrapper) -> Vec<DirEntry> {
        WalkDir::new(Into::<PathBuf>::into(directory_path))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|f| f.file_type().is_file())
            .collect()
    }

    /// Walks the given directory and stores files using par_iter.
    /// If self.target is Some, it will only store files that
    /// are in a directory from target.
    pub fn walk_dir_and_store_files(
        &mut self,
        directory_path: PathWrapper,
    ) -> Result<(), eyre::Error> {
        let target = self.target;
        let dir_reader = Arc::new(Mutex::new(self));
        Self::walk_dir(directory_path)
            .par_iter()
            .map(|entry| {
                let full_path = entry.path();
                if let Some(target) = &target {
                    if target.iter().any(|t| {
                        full_path
                            .ancestors()
                            .any(|a| a.to_str().map(|s| s == t).unwrap_or_default())
                    }) {
                        return Ok(());
                    }
                }
                let path = path_to_vec_string(full_path)?;
                dir_reader
                    .lock()
                    .map_err(|_| eyre::eyre!("Unable to lock reader"))?
                    .insert_file(path_relative_to(path, ROOT), full_path.to_path_buf().into());
                Ok(())
            })
            .collect::<Result<Vec<()>, eyre::Error>>()?;
        Ok(())
    }

    /// Inserts a file into the `DirReader` by recursively navigating the file's
    /// path and inserting the file into the correct sub directory.
    fn insert_file(&mut self, current_path: Vec<String>, full_path: PathWrapper) {
        if current_path.len() > 1 {
            let root_name = current_path.first().cloned().unwrap(); // safe unwrap
            let sub_node = self.sub_dirs.entry(root_name).or_insert_with(|| Self {
                sub_dirs: BTreeMap::default(),
                files: Vec::default(),
                target: self.target,
            });
            sub_node.insert_file(current_path.into_iter().skip(1).collect(), full_path);
        } else {
            self.files.push(full_path);
        }
    }
}

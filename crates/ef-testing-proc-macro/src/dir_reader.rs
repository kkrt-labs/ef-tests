use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use walkdir::WalkDir;

use crate::constants::ROOT;
use crate::constants::TEST_FILTER_PATH;
use crate::filter::Filter;
use crate::utils::path_to_vec_string;
use crate::utils::trim_path_at_folder;

/// The DirReader will iterate all folders and
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
            filter: Arc::new(Filter::new(TEST_FILTER_PATH.clone())),
        }
    }

    /// Walks the given directory and stores all files
    pub fn walk_directory(mut self, directory_path: PathWrapper) -> Result<Self, eyre::Error> {
        for entry in WalkDir::new(Into::<PathBuf>::into(directory_path))
            .into_iter()
            .filter_map(Result::ok)
            .filter(|f| f.file_type().is_file())
        {
            let full_path = entry.path();
            let path = path_to_vec_string(full_path)?;
            self.insert_file(
                trim_path_at_folder(path, ROOT),
                full_path.to_path_buf().into(),
            );
        }
        Ok(self)
    }

    /// Inserts a file into the DirReader by recursively navigating the file's
    /// path and inserting the file into the correct sub directory.
    fn insert_file(&mut self, current_path: Vec<String>, full_path: PathWrapper) {
        if current_path.len() > 1 {
            let root_name = current_path.first().cloned().unwrap(); // safe unwrap
            let sub_node = self.sub_dirs.entry(root_name).or_insert_with(|| DirReader {
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

#[derive(Clone, Debug, Default)]
pub struct PathWrapper(PathBuf);

impl From<PathBuf> for PathWrapper {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl From<PathWrapper> for PathBuf {
    fn from(path: PathWrapper) -> Self {
        path.0
    }
}

impl PathWrapper {
    pub fn read_file_to_string(&self) -> std::io::Result<String> {
        let mut content = String::new();
        std::fs::File::open(&self.0)?.read_to_string(&mut content)?;
        Ok(content)
    }

    pub fn parent(&self) -> Self {
        Self(
            self.0
                .parent()
                .expect("Error getting path's parent")
                .to_path_buf(),
        )
    }

    pub fn file_stem_to_string(&self) -> String {
        self.0
            .file_stem()
            .expect("Error getting path's file name")
            .to_string_lossy()
            .to_string()
    }
}

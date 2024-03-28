use std::{ops::Deref, ops::DerefMut, path::PathBuf};

#[derive(Clone, Debug, Default)]
pub struct PathWrapper(PathBuf);

impl From<PathBuf> for PathWrapper {
    fn from(path: PathBuf) -> Self {
        Self(path)
    }
}

impl Deref for PathWrapper {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PathWrapper {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PathWrapper {
    pub fn read_file_to_string(&self) -> std::io::Result<String> {
        std::fs::read_to_string(&self.0)
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

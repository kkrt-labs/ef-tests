use std::{io::Read, path::PathBuf};

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

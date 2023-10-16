use std::path::Path;

pub(crate) fn path_to_vec_string(path: &Path) -> Result<Vec<String>, eyre::Error> {
    path.iter()
        .map(|os_str| {
            Ok(os_str
                .to_str()
                .ok_or_else(|| eyre::eyre!("Error while converting the path to a string"))?
                .to_string())
        })
        .collect()
}

pub(crate) fn trim_path_at_folder(path: Vec<String>, folder: &str) -> Vec<String> {
    path.into_iter()
        .skip_while(|x| x != folder)
        .skip(1)
        .collect()
}

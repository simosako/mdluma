use std::path::PathBuf;

pub const SCITER_DLL_NAME: &str = "sciter.dll";

#[allow(dead_code)]
pub fn application_icon_path() -> PathBuf {
    PathBuf::from("assets").join("app-icon.ico")
}

#[allow(dead_code)]
pub fn relative_distribution_prerequisite_paths() -> Vec<PathBuf> {
    std::iter::once(PathBuf::from(SCITER_DLL_NAME)).collect()
}

#[cfg(test)]
mod tests {
    use super::{relative_distribution_prerequisite_paths, SCITER_DLL_NAME};
    use std::path::Path;

    #[test]
    fn distribution_prerequisites_include_only_runtime_dll() {
        let distribution_dir = Path::new("dist");
        let prerequisites: Vec<_> = relative_distribution_prerequisite_paths()
            .into_iter()
            .map(|relative_path| distribution_dir.join(relative_path))
            .collect();

        assert_eq!(prerequisites.len(), 1);
        assert_eq!(prerequisites[0], distribution_dir.join(SCITER_DLL_NAME));
    }
}

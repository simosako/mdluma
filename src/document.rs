use crate::ViewerError;
use std::fs;
use std::path::{Path, PathBuf};

pub trait DocumentLoader {
    fn load(&self, path: &Path) -> Result<SourceDocument, ViewerError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    pub path: PathBuf,
    pub file_name: String,
    pub base_dir: PathBuf,
    pub markdown: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct FileDocumentLoader;

fn resolve_document_path(path: &Path) -> Result<PathBuf, ViewerError> {
    if path.as_os_str().is_empty() {
        crate::debug_log!("rejecting empty document path");
        return Err(ViewerError::file_read(path, "empty document path"));
    }
    let resolved = std::path::absolute(path).map_err(|error| {
        crate::debug_log!("path resolution failed for {:?}: {}", path, error);
        ViewerError::file_read(path, error.to_string())
    })?;
    Ok(resolved)
}

impl DocumentLoader for FileDocumentLoader {
    fn load(&self, path: &Path) -> Result<SourceDocument, ViewerError> {
        let path = resolve_document_path(path)?;
        let bytes =
            fs::read(&path).map_err(|error| ViewerError::file_read(&path, error.to_string()))?;
        let markdown =
            String::from_utf8(bytes).map_err(|_| ViewerError::invalid_encoding(&path))?;

        Ok(SourceDocument {
            path: path.clone(),
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            base_dir: path.parent().map(Path::to_path_buf).unwrap_or_default(),
            markdown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{DocumentLoader, FileDocumentLoader};
    use crate::test_util::unique_test_dir;
    use crate::ViewerError;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn loads_local_markdown_as_utf8_with_display_metadata() {
        let dir = unique_test_dir("document-loader-success");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("notes.md");
        let markdown = "# Title\n\nBody";
        fs::write(&path, markdown).expect("write markdown");

        let document = FileDocumentLoader.load(&path).expect("load document");

        assert_eq!(document.path, path);
        assert_eq!(document.file_name, "notes.md");
        assert_eq!(document.base_dir.as_path(), dir.as_ref());
        assert_eq!(document.markdown, markdown);
    }

    #[test]
    fn load_always_produces_absolute_document_path() {
        let dir = unique_test_dir("document-loader-abs-path");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("notes.md");
        let markdown = "# Title\n\nBody";
        fs::write(&path, markdown).expect("write markdown");

        let document = FileDocumentLoader.load(&path).expect("load document");

        assert!(document.path.is_absolute(), "path must be absolute");
        assert_eq!(document.path, path);
    }

    #[test]
    fn load_with_absolute_path_produces_correct_base_dir_and_filename() {
        let dir = unique_test_dir("document-loader-dotdot-basedir");
        fs::create_dir_all(&dir).expect("create test dir");
        let child = dir.join("subdir");
        fs::create_dir_all(&child).expect("create subdir");
        let path = child.join("notes.md");
        let markdown = "# Title";
        fs::write(&path, markdown).expect("write markdown");

        let document = FileDocumentLoader.load(&path).expect("load document");

        assert!(document.base_dir.is_absolute(), "base_dir must be absolute");
        assert_eq!(document.base_dir, child);
        assert_eq!(document.file_name, "notes.md");
    }

    #[test]
    fn same_file_via_two_paths_produces_same_path_and_base_dir() {
        let dir = unique_test_dir("document-loader-same-file");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("notes.md");
        let markdown = "# Title";
        fs::write(&path, markdown).expect("write markdown");

        let absolute = std::path::absolute(&path).expect("absolute");
        let doc1 = FileDocumentLoader.load(&path).expect("load first");
        let doc2 = FileDocumentLoader.load(&absolute).expect("load second");

        assert_eq!(doc1.path, doc2.path);
        assert_eq!(doc1.base_dir, doc2.base_dir);
    }

    #[test]
    fn file_name_is_leaf_of_absolutized_path() {
        let dir = unique_test_dir("document-loader-filename-abs");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("notes.md");
        fs::write(&path, "# Title").expect("write markdown");

        let document = FileDocumentLoader.load(&path).expect("load document");

        assert_eq!(document.file_name, "notes.md");
        assert_eq!(
            document.file_name,
            document
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        );
    }

    #[test]
    fn relative_path_is_absolutized_before_file_operations() {
        let cwd = std::env::current_dir().expect("current dir");
        let test_root = cwd.join("target").join("tmp-doc-tests");
        fs::create_dir_all(&test_root).expect("create test root");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let dir = test_root.join(format!("rel-{nonce}"));
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("notes.md");
        let markdown = "# Title\n\nBody";
        fs::write(&path, markdown).expect("write markdown");

        let relative = path
            .strip_prefix(&cwd)
            .expect("path under cwd")
            .to_path_buf();
        let expected_absolute = std::path::absolute(&relative).expect("absolute");

        let document = FileDocumentLoader.load(&relative).expect("load document");

        assert!(document.path.is_absolute(), "path must be absolute");
        assert_eq!(document.path, expected_absolute);
        assert!(document.base_dir.is_absolute(), "base_dir must be absolute");
        assert_eq!(
            document.base_dir,
            expected_absolute.parent().expect("parent")
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn returns_typed_error_for_missing_file_without_creating_it() {
        let dir = unique_test_dir("document-loader-missing");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("missing.md");

        let error = FileDocumentLoader
            .load(&path)
            .expect_err("missing file should fail");

        assert!(
            matches!(error, ViewerError::FileRead { path: error_path, .. } if error_path == path)
        );
        assert!(!path.exists());
    }

    #[test]
    fn returns_typed_error_for_invalid_utf8_without_overwriting_source() {
        let dir = unique_test_dir("document-loader-invalid-utf8");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("invalid.md");
        let bytes = [0xff, 0xfe, b'#', b' ', b'x'];
        fs::write(&path, bytes).expect("write invalid utf8");

        let error = FileDocumentLoader
            .load(&path)
            .expect_err("invalid UTF-8 should fail");

        assert_eq!(error, ViewerError::invalid_encoding(&path));
        assert_eq!(fs::read(&path).expect("read invalid utf8 source"), bytes);
    }

    #[test]
    #[cfg(windows)]
    fn returns_typed_error_for_permission_failure_without_overwriting_source() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = unique_test_dir("document-loader-access-denied");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("private.md");
        let markdown = "# Private";
        fs::write(&path, markdown).expect("write markdown");
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .expect("open file without sharing");

        let error = FileDocumentLoader
            .load(&path)
            .expect_err("permission failure should fail");

        drop(locked);
        assert!(
            matches!(error, ViewerError::FileRead { path: error_path, .. } if error_path == path)
        );
        assert_eq!(fs::read_to_string(&path).expect("read source"), markdown);
    }

    #[test]
    #[cfg(unix)]
    fn returns_typed_error_for_permission_failure_without_overwriting_source() {
        use std::os::unix::fs::PermissionsExt;

        let dir = unique_test_dir("document-loader-permission");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("private.md");
        let markdown = "# Private";
        fs::write(&path, markdown).expect("write markdown");
        let original_permissions = fs::metadata(&path).expect("metadata").permissions();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000))
            .expect("remove read permission");

        let error = FileDocumentLoader
            .load(&path)
            .expect_err("permission failure should fail");

        fs::set_permissions(&path, original_permissions).expect("restore permissions");
        assert!(
            matches!(error, ViewerError::FileRead { path: error_path, .. } if error_path == path)
        );
        assert_eq!(
            fs::read_to_string(&path).expect("read restored source"),
            markdown
        );
    }

    #[test]
    fn empty_string_path_returns_file_read_error() {
        let error = FileDocumentLoader
            .load(Path::new(""))
            .expect_err("empty path should fail");

        assert!(
            matches!(error, ViewerError::FileRead { path: error_path, .. } if error_path == PathBuf::new())
        );
    }

    #[test]
    fn empty_pathbuf_returns_file_read_error() {
        let error = FileDocumentLoader
            .load(Path::new(""))
            .expect_err("empty path should fail");

        assert!(matches!(error, ViewerError::FileRead { .. }));
    }

    #[test]
    fn dotdot_in_input_path_resolves_to_correct_absolute_path_and_base_dir() {
        let dir = unique_test_dir("document-loader-dotdot-resolve");
        let target_dir = dir.join("target");
        let sibling_dir = dir.join("sibling");
        fs::create_dir_all(&target_dir).expect("create target dir");
        fs::create_dir_all(&sibling_dir).expect("create sibling dir");
        let file_path = target_dir.join("notes.md");
        let markdown = "# DotDot";
        fs::write(&file_path, markdown).expect("write markdown");

        let dotdot_path = sibling_dir.join("..").join("target").join("notes.md");
        let expected_absolute = std::path::absolute(&dotdot_path).expect("absolute");

        let document = FileDocumentLoader
            .load(&dotdot_path)
            .expect("load document");

        assert!(document.path.is_absolute(), "path must be absolute");
        assert_eq!(document.path, expected_absolute);
        assert_eq!(
            document.base_dir,
            expected_absolute.parent().expect("parent")
        );
        assert_eq!(document.file_name, "notes.md");
        assert_eq!(document.markdown, markdown);
    }

    #[test]
    fn dot_in_input_path_resolves_to_correct_absolute_path_and_base_dir() {
        let dir = unique_test_dir("document-loader-dot-resolve");
        let subdir = dir.join("subdir");
        fs::create_dir_all(&subdir).expect("create subdir");
        let file_path = subdir.join("notes.md");
        let markdown = "# Dot";
        fs::write(&file_path, markdown).expect("write markdown");

        let dot_path = dir.join(".").join("subdir").join("notes.md");
        let expected_absolute = std::path::absolute(&dot_path).expect("absolute");

        let document = FileDocumentLoader.load(&dot_path).expect("load document");

        assert!(document.path.is_absolute(), "path must be absolute");
        assert_eq!(document.path, expected_absolute);
        assert_eq!(
            document.base_dir,
            expected_absolute.parent().expect("parent")
        );
        assert_eq!(document.file_name, "notes.md");
        assert_eq!(document.markdown, markdown);
    }

    #[test]
    fn dotdot_chained_in_input_path_resolves_correctly() {
        let dir = unique_test_dir("document-loader-dotdot-chain");
        let a = dir.join("a").join("b").join("c");
        let target = dir.join("target");
        fs::create_dir_all(&a).expect("create deep dir");
        fs::create_dir_all(&target).expect("create target dir");
        let file_path = target.join("deep.md");
        let markdown = "# Deep";
        fs::write(&file_path, markdown).expect("write markdown");

        let dotdot_path = a
            .join("..")
            .join("..")
            .join("..")
            .join("target")
            .join("deep.md");
        let expected_absolute = std::path::absolute(&dotdot_path).expect("absolute");

        let document = FileDocumentLoader
            .load(&dotdot_path)
            .expect("load document");

        assert!(document.path.is_absolute(), "path must be absolute");
        assert_eq!(document.path, expected_absolute);
        assert_eq!(
            document.base_dir,
            expected_absolute.parent().expect("parent")
        );
        assert_eq!(document.file_name, "deep.md");
    }

    #[test]
    fn loading_document_does_not_modify_source_file() {
        let dir = unique_test_dir("document-loader-readonly");
        fs::create_dir_all(&dir).expect("create test dir");
        let path = dir.join("readonly.md");
        let markdown = "# Read only\n\nDo not change.";
        fs::write(&path, markdown).expect("write markdown");
        let before = fs::metadata(&path).expect("metadata before");

        let document = FileDocumentLoader.load(&path).expect("load document");
        let after = fs::metadata(&path).expect("metadata after");

        assert_eq!(document.markdown, markdown);
        assert_eq!(fs::read_to_string(&path).expect("read source"), markdown);
        assert_eq!(after.len(), before.len());
        assert_eq!(
            after.modified().expect("modified after"),
            before.modified().expect("modified before")
        );
    }
}

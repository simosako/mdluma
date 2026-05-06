pub const MAX_OPEN_FILE_COUNT: usize = 10;

pub struct DropOpenPlan {
    pub current_path: Option<std::path::PathBuf>,
    pub child_paths: Vec<std::path::PathBuf>,
}

pub fn plan_drop_open(paths: Vec<std::path::PathBuf>) -> DropOpenPlan {
    let mut files: Vec<std::path::PathBuf> = paths
        .into_iter()
        .filter(|path| {
            std::fs::metadata(path)
                .map(|m| m.is_file())
                .unwrap_or(false)
        })
        .take(MAX_OPEN_FILE_COUNT)
        .collect();

    let current_path = files.first().cloned();
    let child_paths = if files.len() > 1 {
        files.split_off(1)
    } else {
        Vec::new()
    };

    DropOpenPlan {
        current_path,
        child_paths,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_file(path: &str) -> PathBuf {
        let file = std::env::temp_dir().join(format!(
            "mdluma-test-{}-{}",
            path.replace(['/', '\\'], "_"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&file, "test").expect("create test file");
        file
    }

    fn make_dir(path: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mdluma-testdir-{}-{}",
            path.replace(['/', '\\'], "_"),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        dir
    }

    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn empty_input_produces_empty_plan() {
        let plan = plan_drop_open(vec![]);
        assert!(plan.current_path.is_none());
        assert!(plan.child_paths.is_empty());
    }

    #[test]
    fn single_file_goes_to_current_path() {
        let file = make_file("single.md");
        let plan = plan_drop_open(vec![file.clone()]);
        assert_eq!(plan.current_path, Some(file.clone()));
        assert!(plan.child_paths.is_empty());
        cleanup(&file);
    }

    #[test]
    fn multiple_files_first_goes_to_current_rest_to_children() {
        let f1 = make_file("multi1.md");
        let f2 = make_file("multi2.md");
        let f3 = make_file("multi3.md");
        let plan = plan_drop_open(vec![f1.clone(), f2.clone(), f3.clone()]);
        assert_eq!(plan.current_path, Some(f1.clone()));
        assert_eq!(plan.child_paths, vec![f2.clone(), f3.clone()]);
        for f in &[&f1, &f2, &f3] {
            cleanup(f);
        }
    }

    #[test]
    fn folders_are_excluded_from_plan() {
        let file = make_file("withfolder-file.md");
        let dir = make_dir("withfolder-dir");
        let plan = plan_drop_open(vec![dir.clone(), file.clone()]);
        assert_eq!(plan.current_path, Some(file.clone()));
        assert!(plan.child_paths.is_empty());
        cleanup(&file);
        cleanup(&dir);
    }

    #[test]
    fn folders_only_produces_empty_plan() {
        let d1 = make_dir("foldersonly1");
        let d2 = make_dir("foldersonly2");
        let plan = plan_drop_open(vec![d1.clone(), d2.clone()]);
        assert!(plan.current_path.is_none());
        assert!(plan.child_paths.is_empty());
        cleanup(&d1);
        cleanup(&d2);
    }

    #[test]
    fn more_than_ten_files_are_capped_at_ten() {
        let files: Vec<PathBuf> = (0..12)
            .map(|i| make_file(&format!("cap{}.md", i)))
            .collect();
        let plan = plan_drop_open(files.clone());
        let total = plan.child_paths.len() + if plan.current_path.is_some() { 1 } else { 0 };
        assert_eq!(total, 10);
        assert_eq!(plan.current_path, Some(files[0].clone()));
        assert_eq!(plan.child_paths.len(), 9);
        for f in &files {
            cleanup(f);
        }
    }

    #[test]
    fn input_order_is_preserved_after_filtering_and_capping() {
        let files: Vec<PathBuf> = (0..3).map(|i| make_file(&format!("ord{}.md", i))).collect();
        let dirs: Vec<PathBuf> = (0..2).map(|i| make_dir(&format!("orddir{}", i))).collect();
        let mixed = vec![
            dirs[0].clone(),
            files[0].clone(),
            files[1].clone(),
            dirs[1].clone(),
            files[2].clone(),
        ];
        let plan = plan_drop_open(mixed);
        assert_eq!(plan.current_path, Some(files[0].clone()));
        assert_eq!(plan.child_paths, vec![files[1].clone(), files[2].clone()]);
        for f in &files {
            cleanup(f);
        }
        for d in &dirs {
            cleanup(d);
        }
    }

    #[test]
    fn mixed_folders_and_files_folders_ignored_files_capped() {
        let files: Vec<PathBuf> = (0..3).map(|i| make_file(&format!("mf{}.md", i))).collect();
        let dirs: Vec<PathBuf> = (0..5).map(|i| make_dir(&format!("mfd{}", i))).collect();
        let mut mixed: Vec<PathBuf> = Vec::new();
        for i in 0..5 {
            mixed.push(dirs[i].clone());
        }
        for f in &files {
            mixed.push(f.clone());
        }
        let plan = plan_drop_open(mixed);
        assert_eq!(plan.current_path, Some(files[0].clone()));
        assert_eq!(plan.child_paths, vec![files[1].clone(), files[2].clone()]);
        for f in &files {
            cleanup(f);
        }
        for d in &dirs {
            cleanup(d);
        }
    }

    #[test]
    fn max_open_file_count_is_ten() {
        assert_eq!(MAX_OPEN_FILE_COUNT, 10);
    }

    #[test]
    fn exactly_ten_files_all_used() {
        let files: Vec<PathBuf> = (0..10)
            .map(|i| make_file(&format!("ex10{}.md", i)))
            .collect();
        let plan = plan_drop_open(files.clone());
        assert_eq!(plan.current_path, Some(files[0].clone()));
        assert_eq!(plan.child_paths.len(), 9);
        for (i, p) in plan.child_paths.iter().enumerate() {
            assert_eq!(*p, files[i + 1]);
        }
        for f in &files {
            cleanup(f);
        }
    }
}

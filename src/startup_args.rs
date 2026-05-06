pub enum StartupNotice {
    UnsupportedOption(std::ffi::OsString),
}

pub enum LaunchAction {
    StartViewer {
        initial_path: Option<std::path::PathBuf>,
    },
    SpawnChildren {
        file_paths: Vec<std::path::PathBuf>,
    },
}

pub struct StartupLaunchPlan {
    pub action: LaunchAction,
    pub notices: Vec<StartupNotice>,
}

use crate::open_paths::MAX_OPEN_FILE_COUNT;

pub fn plan_startup_launch<I>(args: I) -> StartupLaunchPlan
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut file_paths = Vec::new();
    let mut notices = Vec::new();

    for arg in args {
        if arg.as_encoded_bytes().starts_with(b"--") {
            notices.push(StartupNotice::UnsupportedOption(arg));
        } else if file_paths.len() < MAX_OPEN_FILE_COUNT {
            file_paths.push(std::path::PathBuf::from(arg));
        }
    }

    let action = match file_paths.len() {
        0 => LaunchAction::StartViewer { initial_path: None },
        1 => LaunchAction::StartViewer {
            initial_path: Some(file_paths.into_iter().next().unwrap()),
        },
        _ => LaunchAction::SpawnChildren { file_paths },
    };

    StartupLaunchPlan { action, notices }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(s: &str) -> std::ffi::OsString {
        std::ffi::OsString::from(s)
    }

    fn path(s: &str) -> std::path::PathBuf {
        std::path::PathBuf::from(s)
    }

    // --- Requirement 1.2: No arguments → empty viewer ---

    #[test]
    fn no_arguments_produces_empty_viewer_with_no_notices() {
        let plan = plan_startup_launch(std::iter::empty::<std::ffi::OsString>());

        assert!(matches!(
            plan.action,
            LaunchAction::StartViewer { initial_path } if initial_path.is_none()
        ));
        assert!(plan.notices.is_empty());
    }

    // --- Requirement 1.1: Single file path → single viewer ---

    #[test]
    fn single_file_path_produces_viewer_with_initial_path() {
        let plan = plan_startup_launch(vec![os("readme.md")]);

        match &plan.action {
            LaunchAction::StartViewer { initial_path } => {
                assert_eq!(
                    initial_path.as_deref(),
                    Some(std::path::Path::new("readme.md"))
                );
            }
            LaunchAction::SpawnChildren { .. } => panic!("expected StartViewer, got SpawnChildren"),
        }
        assert!(plan.notices.is_empty());
    }

    // --- Requirement 2.1: 2-10 files → spawn children ---

    #[test]
    fn two_files_produce_spawn_children() {
        let plan = plan_startup_launch(vec![os("a.md"), os("b.md")]);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 2);
                assert_eq!(file_paths[0], path("a.md"));
                assert_eq!(file_paths[1], path("b.md"));
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren, got StartViewer"),
        }
        assert!(plan.notices.is_empty());
    }

    #[test]
    fn ten_files_produce_spawn_children_with_all_ten() {
        let args: Vec<std::ffi::OsString> = (0..10).map(|i| os(&format!("file{}.md", i))).collect();
        let plan = plan_startup_launch(args);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 10);
                for (i, p) in file_paths.iter().enumerate() {
                    assert_eq!(p, &path(&format!("file{}.md", i)));
                }
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren, got StartViewer"),
        }
    }

    // --- Requirements 3.1, 3.2: 11+ files → first 10 only ---

    #[test]
    fn eleven_files_keeps_only_first_ten() {
        let args: Vec<std::ffi::OsString> = (0..11).map(|i| os(&format!("file{}.md", i))).collect();
        let plan = plan_startup_launch(args);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 10);
                assert_eq!(file_paths[0], path("file0.md"));
                assert_eq!(file_paths[9], path("file9.md"));
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren, got StartViewer"),
        }
    }

    #[test]
    fn many_files_are_silently_ignored_beyond_ten() {
        let args: Vec<std::ffi::OsString> = (0..25).map(|i| os(&format!("file{}.md", i))).collect();
        let plan = plan_startup_launch(args);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 10);
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren, got StartViewer"),
        }
        assert!(plan.notices.is_empty());
    }

    // --- Requirement 4.1: -- prefixed args are not file paths ---

    #[test]
    fn double_dash_argument_is_classified_as_unsupported_option() {
        let plan = plan_startup_launch(vec![os("--bad")]);

        assert!(matches!(
            plan.action,
            LaunchAction::StartViewer { initial_path } if initial_path.is_none()
        ));
        assert_eq!(plan.notices.len(), 1);
        assert!(matches!(
            &plan.notices[0],
            StartupNotice::UnsupportedOption(opt) if opt == "--bad"
        ));
    }

    #[test]
    fn double_dash_arguments_do_not_appear_as_file_paths() {
        let plan = plan_startup_launch(vec![
            os("--verbose"),
            os("readme.md"),
            os("--output=out.html"),
        ]);

        match &plan.action {
            LaunchAction::StartViewer { initial_path } => {
                assert_eq!(
                    initial_path.as_deref(),
                    Some(std::path::Path::new("readme.md"))
                );
            }
            LaunchAction::SpawnChildren { .. } => panic!("expected StartViewer"),
        }
        assert_eq!(plan.notices.len(), 2);
        assert!(matches!(
            &plan.notices[0],
            StartupNotice::UnsupportedOption(opt) if opt == "--verbose"
        ));
        assert!(matches!(
            &plan.notices[1],
            StartupNotice::UnsupportedOption(opt) if opt == "--output=out.html"
        ));
    }

    // --- Requirement 4.3: Mixed unsupported + valid files → files preserved ---

    #[test]
    fn mixed_unsupported_and_file_paths_preserves_file_order() {
        let plan = plan_startup_launch(vec![
            os("--flag"),
            os("first.md"),
            os("second.md"),
            os("--another"),
        ]);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 2);
                assert_eq!(file_paths[0], path("first.md"));
                assert_eq!(file_paths[1], path("second.md"));
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren"),
        }
        assert_eq!(plan.notices.len(), 2);
    }

    // --- Requirement 4.4: Only unsupported options → empty viewer ---

    #[test]
    fn only_unsupported_options_produces_empty_viewer() {
        let plan = plan_startup_launch(vec![os("--help"), os("--version")]);

        assert!(matches!(
            plan.action,
            LaunchAction::StartViewer { initial_path } if initial_path.is_none()
        ));
        assert_eq!(plan.notices.len(), 2);
    }

    // --- Invariant: input order preserved ---

    #[test]
    fn file_path_input_order_is_preserved() {
        let plan = plan_startup_launch(vec![os("c.md"), os("a.md"), os("b.md")]);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths, &[path("c.md"), path("a.md"), path("b.md")]);
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren"),
        }
    }

    // --- Invariant: SpawnChildren file_paths is always 2..=10 ---

    #[test]
    fn spawn_children_always_has_two_to_ten_paths() {
        for count in 2..=10 {
            let args: Vec<std::ffi::OsString> =
                (0..count).map(|i| os(&format!("f{}.md", i))).collect();
            let plan = plan_startup_launch(args);
            match &plan.action {
                LaunchAction::SpawnChildren { file_paths } => {
                    assert_eq!(file_paths.len(), count);
                    assert!((2..=10).contains(&file_paths.len()));
                }
                LaunchAction::StartViewer { .. } => {
                    panic!("expected SpawnChildren for {count} files")
                }
            }
        }
    }

    // --- Edge case: single dash is a file path (not an option) ---

    #[test]
    fn single_dash_is_treated_as_file_path() {
        let plan = plan_startup_launch(vec![os("-")]);

        match &plan.action {
            LaunchAction::StartViewer { initial_path } => {
                assert_eq!(initial_path.as_deref(), Some(std::path::Path::new("-")));
            }
            LaunchAction::SpawnChildren { .. } => panic!("expected StartViewer"),
        }
        assert!(plan.notices.is_empty());
    }

    // --- Edge case: argument starting with single dash ---

    #[test]
    fn single_dash_prefix_is_treated_as_file_path() {
        let plan = plan_startup_launch(vec![os("-v")]);

        match &plan.action {
            LaunchAction::StartViewer { initial_path } => {
                assert_eq!(initial_path.as_deref(), Some(std::path::Path::new("-v")));
            }
            LaunchAction::SpawnChildren { .. } => panic!("expected StartViewer"),
        }
        assert!(plan.notices.is_empty());
    }

    // --- Req 4.3 boundary: single file survives among unsupported options ---

    #[test]
    fn single_file_with_surrounding_unsupported_options_still_opens_viewer() {
        let plan = plan_startup_launch(vec![os("--alpha"), os("solo.md"), os("--beta")]);

        match &plan.action {
            LaunchAction::StartViewer { initial_path } => {
                assert_eq!(
                    initial_path.as_deref(),
                    Some(std::path::Path::new("solo.md"))
                );
            }
            LaunchAction::SpawnChildren { .. } => panic!("expected StartViewer for 1 valid file"),
        }
        assert_eq!(plan.notices.len(), 2);
    }

    // --- Req 4.3 + 3.1: unsupported options don't count toward file cap ---

    #[test]
    fn unsupported_options_do_not_consume_file_slots() {
        let mut args = vec![os("--flag1"), os("--flag2")];
        for i in 0..10 {
            args.push(os(&format!("file{}.md", i)));
        }
        args.push(os("--flag3"));
        args.push(os("extra.md"));
        let plan = plan_startup_launch(args);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 10);
                assert_eq!(file_paths[0], path("file0.md"));
                assert_eq!(file_paths[9], path("file9.md"));
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren"),
        }
        assert_eq!(plan.notices.len(), 3);
    }

    // --- Req 4.1: notice order matches input order of unsupported args ---

    #[test]
    fn unsupported_option_notices_preserve_input_order() {
        let plan = plan_startup_launch(vec![
            os("--gamma"),
            os("a.md"),
            os("--alpha"),
            os("--beta"),
            os("b.md"),
        ]);

        assert_eq!(plan.notices.len(), 3);
        assert!(matches!(
            &plan.notices[0],
            StartupNotice::UnsupportedOption(opt) if opt == "--gamma"
        ));
        assert!(matches!(
            &plan.notices[1],
            StartupNotice::UnsupportedOption(opt) if opt == "--alpha"
        ));
        assert!(matches!(
            &plan.notices[2],
            StartupNotice::UnsupportedOption(opt) if opt == "--beta"
        ));
    }

    // --- Req 3.1 + 4.3: interleaved files and options with 11+ files ---

    #[test]
    fn interleaved_unsupported_options_with_over_ten_files_keeps_first_ten_files() {
        let plan = plan_startup_launch(vec![
            os("--opt1"),
            os("f0.md"),
            os("--opt2"),
            os("f1.md"),
            os("f2.md"),
            os("f3.md"),
            os("--opt3"),
            os("f4.md"),
            os("f5.md"),
            os("f6.md"),
            os("f7.md"),
            os("f8.md"),
            os("--opt4"),
            os("f9.md"),
            os("f10.md"),
            os("f11.md"),
        ]);

        match &plan.action {
            LaunchAction::SpawnChildren { file_paths } => {
                assert_eq!(file_paths.len(), 10);
                assert_eq!(file_paths[0], path("f0.md"));
                assert_eq!(file_paths[9], path("f9.md"));
            }
            LaunchAction::StartViewer { .. } => panic!("expected SpawnChildren"),
        }
        assert_eq!(plan.notices.len(), 4);
    }

    // --- Req 4.4: multiple unsupported options with no files ---

    #[test]
    fn three_unsupported_options_with_no_files_preserves_all_notices() {
        let plan = plan_startup_launch(vec![os("--help"), os("--unknown"), os("--version")]);

        assert!(matches!(
            plan.action,
            LaunchAction::StartViewer { initial_path } if initial_path.is_none()
        ));
        assert_eq!(plan.notices.len(), 3);
        assert!(matches!(
            &plan.notices[0],
            StartupNotice::UnsupportedOption(opt) if opt == "--help"
        ));
        assert!(matches!(
            &plan.notices[1],
            StartupNotice::UnsupportedOption(opt) if opt == "--unknown"
        ));
        assert!(matches!(
            &plan.notices[2],
            StartupNotice::UnsupportedOption(opt) if opt == "--version"
        ));
    }
}

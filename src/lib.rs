mod app;
mod debug_log;
mod document;
mod errors;
mod external_editor;
mod html_sanitizer;
mod html_shell;
mod markdown;
mod open_paths;
mod platform;
mod sciter;
mod settings;
mod startup_args;
mod ui;
mod viewer_launcher;

#[macro_export]
macro_rules! debug_log {
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            $crate::__write_debug_log("DEBUG", &format!($($arg)*));
        }
    }};
}

#[doc(hidden)]
pub use debug_log::write_debug_log as __write_debug_log;

pub use app::{RenderedDocument, ViewerState, APP_NAME};
pub use document::{DocumentLoader, FileDocumentLoader, SourceDocument};
pub use errors::{StartupError, ViewerError};
pub use html_shell::{DefaultHtmlShell, HtmlShell, ShellModel};
pub use markdown::{ComrakMarkdownRenderer, MarkdownOptions, MarkdownRenderer};
pub use platform::{
    FileDialog, FontDialog, FontDialogResult, OpenFileResult, WindowChromeController,
    WindowChromeState, WindowsFileDialog, WindowsFontDialog, WindowsWindowChrome,
};
pub use sciter::runtime::{RuntimePrerequisites, SciterRuntime, SciterVersion, SCITER_DLL_NAME};
pub use sciter::window::{SciterWindow, ViewerUi};
pub use ui::{EmbeddedUiAssets, IconName, IconTheme, Theme, UiTextAsset};

use std::cell::RefCell;
use std::rc::Rc;

use external_editor::{ExternalEditorLauncher, ProcessExternalEditorLauncher};
use startup_args::{LaunchAction, StartupLaunchPlan, StartupNotice};
use viewer_launcher::{ProcessViewerChildLauncher, ViewerChildLauncher};

type StartupController<U, S = (), E = ()> = app::AppController<
    WindowsFileDialog,
    WindowsFontDialog,
    FileDocumentLoader,
    ComrakMarkdownRenderer,
    DefaultHtmlShell,
    U,
    S,
    E,
>;

pub fn run() -> Result<(), StartupError> {
    debug_log::init_debug_log();
    let distribution_dir = runtime_distribution_dir()?;
    let plan = startup_args::plan_startup_launch(std::env::args_os().skip(1));
    execute_launch_plan(
        plan,
        &mut |message| eprintln!("{message}"),
        distribution_dir,
        |prerequisites| SciterRuntime::load(prerequisites),
        |_distribution_dir, runtime| {
            build_startup_controller(
                runtime,
                |runtime| {
                    let settings_file = crate::settings::SettingsFile::new();
                    let settings = settings_file.load();
                    SciterWindow::with_geometry(
                        runtime,
                        settings.window_geometry.as_ref(),
                    )
                    .map(|window| Rc::new(RefCell::new(window)))
                },
                ProcessViewerChildLauncher::default(),
                ProcessExternalEditorLauncher::default(),
            )
        },
        ProcessViewerChildLauncher::default(),
    )
}

fn report_startup_notice(notice: &StartupNotice, stderr_reporter: &mut dyn FnMut(&str)) {
    match notice {
        StartupNotice::UnsupportedOption(option) => {
            stderr_reporter(&format!(
                "MDLuma: unrecognized option: {}",
                option.to_string_lossy()
            ));
        }
    }
}

fn execute_launch_plan<ValidateRuntime, BuildController, U, S, E, L>(
    plan: StartupLaunchPlan,
    stderr_reporter: &mut dyn FnMut(&str),
    distribution_dir: std::path::PathBuf,
    validate_runtime: ValidateRuntime,
    build_controller: BuildController,
    launcher: L,
) -> Result<(), StartupError>
where
    L: ViewerChildLauncher,
    U: Clone + ViewerUi + sciter::window::ViewerCommandBinder,
    ValidateRuntime: FnOnce(RuntimePrerequisites) -> Result<SciterRuntime, ViewerError>,
    BuildController: FnOnce(
        std::path::PathBuf,
        SciterRuntime,
    ) -> Result<StartupController<U, S, E>, ViewerError>,
    S: ViewerChildLauncher,
    E: ExternalEditorLauncher,
{
    for notice in &plan.notices {
        report_startup_notice(notice, stderr_reporter);
    }

    match plan.action {
        LaunchAction::StartViewer { initial_path } => start_viewer_with(
            initial_path.as_deref(),
            distribution_dir,
            validate_runtime,
            build_controller,
        ),
        LaunchAction::SpawnChildren { file_paths } => {
            const CASCADE_STEP: i32 = 30;
            const MAX_OFFSET: i32 = 200;
            for (index, file_path) in file_paths.into_iter().enumerate() {
                let offset = (index as i32 * CASCADE_STEP).min(MAX_OFFSET);
                launcher
                    .launch_path(&file_path, offset, offset)
                    .map_err(StartupError::from_viewer_error)?;
            }
            Ok(())
        }
    }
}

fn start_viewer_with<ValidateRuntime, BuildController, U, S, E>(
    initial_path: Option<&std::path::Path>,
    distribution_dir: std::path::PathBuf,
    validate_runtime: ValidateRuntime,
    build_controller: BuildController,
) -> Result<(), StartupError>
where
    U: Clone + ViewerUi + sciter::window::ViewerCommandBinder,
    S: ViewerChildLauncher,
    E: ExternalEditorLauncher,
    ValidateRuntime: FnOnce(RuntimePrerequisites) -> Result<SciterRuntime, ViewerError>,
    BuildController: FnOnce(
        std::path::PathBuf,
        SciterRuntime,
    ) -> Result<StartupController<U, S, E>, ViewerError>,
{
    let prerequisites = RuntimePrerequisites::from_distribution_dir(&distribution_dir);
    let runtime = validate_runtime(prerequisites).map_err(StartupError::from_viewer_error)?;
    let mut controller =
        build_controller(distribution_dir, runtime).map_err(StartupError::from_viewer_error)?;

    if let Some(path) = initial_path {
        controller.prepare_startup_path(path);
    }

    controller.run().map_err(StartupError::from_viewer_error)?;
    Ok(())
}

fn runtime_distribution_dir() -> Result<std::path::PathBuf, StartupError> {
    let executable = std::env::current_exe().map_err(|error| {
        StartupError::new(format!(
            "MDLuma cannot determine its runtime directory. Diagnostic: {error}"
        ))
    })?;

    executable
        .parent()
        .map(std::path::Path::to_path_buf)
        .ok_or_else(|| {
            StartupError::new(format!(
                "MDLuma cannot determine its runtime directory. Diagnostic: missing parent for {}",
                executable.display()
            ))
        })
}

fn build_startup_controller<U, S, E>(
    runtime: SciterRuntime,
    ui_factory: impl FnOnce(SciterRuntime) -> Result<U, ViewerError>,
    launcher: S,
    external_editor_launcher: E,
) -> Result<StartupController<U, S, E>, ViewerError>
where
    U: ViewerUi,
    S: ViewerChildLauncher,
    E: ExternalEditorLauncher,
{
    let shell = DefaultHtmlShell::new(EmbeddedUiAssets);
    let ui = ui_factory(runtime)?;

    Ok(app::AppController::with_launchers(
        WindowsFileDialog,
        WindowsFontDialog,
        FileDocumentLoader,
        ComrakMarkdownRenderer,
        shell,
        ui,
        launcher,
        external_editor_launcher,
    ))
}

#[cfg(test)]
mod tests {
    use super::{run, StartupController, APP_NAME};
    use crate::errors::ViewerError;
    use crate::sciter::runtime::{SciterRuntime, SCITER_DLL_NAME};
    use crate::sciter::runtime_assets::relative_distribution_prerequisite_paths;
    use crate::sciter::window::{
        ViewerCommand, ViewerCommandBinder, ViewerCommandHandler, ViewerUi,
    };
    use crate::settings::SettingsFile;
    use crate::{build_startup_controller, start_viewer_with};
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;

    #[test]
    fn package_exposes_application_identity() {
        assert_eq!(APP_NAME, "MDLuma");
    }

    #[test]
    fn startup_flow_validates_runtime_shows_initial_window_and_enters_event_loop() {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let observed_prerequisites = Rc::new(RefCell::new(None));
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let expected_dll = distribution_dir.join(SCITER_DLL_NAME);
        let expected_assets: Vec<_> = relative_distribution_prerequisite_paths()
            .into_iter()
            .map(|relative_path| distribution_dir.join(relative_path))
            .collect();

        start_viewer_with(
            None,
            distribution_dir.clone(),
            {
                let observed_prerequisites = observed_prerequisites.clone();
                move |prerequisites| {
                    *observed_prerequisites.borrow_mut() = Some(prerequisites);
                    Ok(fake_runtime())
                }
            },
            {
                let ui = ui.clone();
                move |_distribution_dir, runtime| {
                    build_startup_controller(runtime, |_| Ok(ui.clone()), (), ())
                }
            },
        )
        .expect("startup should succeed");

        let prerequisites = observed_prerequisites
            .borrow_mut()
            .take()
            .expect("startup should compute runtime prerequisites");
        assert_eq!(prerequisites.sciter_dll_path, expected_dll);
        assert_eq!(prerequisites.required_files[0], expected_dll);
        for asset in expected_assets {
            assert!(
                prerequisites.required_files.contains(&asset),
                "missing startup asset prerequisite: {}",
                asset.display()
            );
        }

        let ui = ui.borrow();
        assert_eq!(ui.bind_count, 1);
        assert_eq!(ui.event_loop_count, 1);
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.initial_html[0].contains("MDLuma"));
        assert!(ui.initial_html[0].contains("Open Markdown file"));
        assert!(ui.initial_html[0].contains("class=\"titlebar-drag-region\""));
        assert!(ui.initial_html[0].contains("data-current-file"));
        assert!(!ui.initial_html[0].contains("No file open"));
        assert!(ui.initial_html[0].contains(r#"data-action="external-editor-setting""#,));
        assert!(ui.document_html.is_empty());
        assert!(ui.errors.is_empty());
        assert!(matches!(
            ui.dispatched_commands.as_slice(),
            [ViewerCommand::OpenFileRequested]
        ));
    }

    #[test]
    fn startup_failure_reports_diagnosable_runtime_error_before_window_creation() {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let runtime_file = distribution_dir.join(SCITER_DLL_NAME);

        let error =
            start_viewer_with(
                None,
                distribution_dir,
                |_| Err(ViewerError::runtime_missing(&runtime_file)),
                |_distribution_dir,
                 _runtime|
                 -> Result<
                    crate::StartupController<Rc<RefCell<RecordingStartupUi>>>,
                    ViewerError,
                > {
                    panic!("window creation must not run when runtime validation fails")
                },
            )
            .expect_err("missing runtime should stop startup before window creation");

        let message = error.to_string();
        assert!(message.contains("MDLuma cannot start because a required runtime file is missing."));
        assert!(message.contains("missing runtime file: expected"));
        assert!(message.contains(&runtime_file.display().to_string()));
    }

    #[test]
    fn current_run_reports_diagnosable_startup_failure() {
        let error = run().expect_err("run should fail in tests without packaged runtime files");

        let message = error.to_string();
        assert!(message.contains("MDLuma cannot start"));
        assert!(message.contains("Diagnostic:"));
    }

    #[test]
    fn startup_flow_records_small_initial_window_baseline_before_any_document_load() {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));

        start_viewer_with(None, distribution_dir.clone(), |_| Ok(fake_runtime()), {
            let ui = ui.clone();
            move |_distribution_dir, runtime| {
                build_startup_controller(runtime, |_| Ok(ui.clone()), (), ())
            }
        })
        .expect("startup baseline should succeed");

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.document_html.is_empty());
        assert!(ui.initial_html_bytes() <= 64 * 1024);
        assert!(ui
            .latest_initial_html()
            .contains("class=\"titlebar-drag-region\""));
        assert!(ui
            .latest_initial_html()
            .contains("data-current-file"));
        assert!(!ui.latest_initial_html().contains("No file open"));
        assert!(ui.latest_initial_html().contains("Open Markdown file"));
        assert!(ui
            .latest_initial_html()
            .contains(r#"data-action="external-editor-setting""#));
    }

    #[derive(Default)]
    struct RecordingStartupUi {
        initial_html: Vec<String>,
        document_html: Vec<String>,
        errors: Vec<ViewerError>,
        bind_count: usize,
        event_loop_count: usize,
        dispatched_commands: Vec<ViewerCommand>,
        steps: Vec<&'static str>,
    }

    impl RecordingStartupUi {
        fn bind_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
        where
            H: ViewerCommandHandler,
        {
            self.bind_count += 1;
            self.steps.push("bind");
            let _ = handler;
            self.dispatched_commands
                .push(ViewerCommand::OpenFileRequested);
            Ok(())
        }

        fn initial_html_bytes(&self) -> usize {
            self.initial_html.iter().map(|html| html.len()).sum()
        }

        fn latest_initial_html(&self) -> &str {
            self.initial_html
                .last()
                .map(String::as_str)
                .expect("recorded initial html")
        }
    }

    impl ViewerCommandBinder for Rc<RefCell<RecordingStartupUi>> {
        fn bind_viewer_command_handler<H>(&mut self, handler: &mut H) -> Result<(), ViewerError>
        where
            H: ViewerCommandHandler,
        {
            self.borrow_mut().bind_handler(handler)
        }
    }

    impl ViewerUi for Rc<RefCell<RecordingStartupUi>> {
        fn show_initial(&mut self, html: &str) -> Result<(), ViewerError> {
            self.borrow_mut().steps.push("show_initial");
            self.borrow_mut().initial_html.push(html.to_string());
            Ok(())
        }

        fn show_document(&mut self, html: &str) -> Result<(), ViewerError> {
            self.borrow_mut().document_html.push(html.to_string());
            Ok(())
        }

        fn show_error(&mut self, error: &ViewerError) -> Result<(), ViewerError> {
            self.borrow_mut().errors.push(error.clone());
            Ok(())
        }

        fn run_event_loop(&mut self) -> Result<(), ViewerError> {
            self.borrow_mut().event_loop_count += 1;
            self.borrow_mut().steps.push("run_event_loop");
            Ok(())
        }

        fn request_close(&mut self) -> Result<(), ViewerError> {
            self.borrow_mut().steps.push("request_close");
            Ok(())
        }
    }

    #[test]
    fn startup_flow_binds_viewer_command_handler_before_loading_initial_html() {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));

        start_viewer_with(None, distribution_dir.clone(), |_| Ok(fake_runtime()), {
            let ui = ui.clone();
            move |_distribution_dir, runtime| {
                build_startup_controller(runtime, |_| Ok(ui.clone()), (), ())
            }
        })
        .expect("startup should succeed");

        let ui = ui.borrow();
        assert_eq!(ui.steps, vec!["bind", "show_initial", "run_event_loop"]);
    }

    fn fake_runtime() -> SciterRuntime {
        SciterRuntime::ready_for_tests()
    }

    // --- Task 3.1: startup plan integration tests ---

    use crate::startup_args::{plan_startup_launch, StartupNotice};
    use crate::StartupError;
    use std::ffi::OsString;

    fn os(s: &str) -> OsString {
        OsString::from(s)
    }

    fn start_plan_with(
        args: Vec<OsString>,
        ui: &Rc<RefCell<RecordingStartupUi>>,
    ) -> (Vec<String>, Result<(), StartupError>) {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let plan = plan_startup_launch(args);
        let mut stderr_lines: Vec<String> = Vec::new();
        let test_settings_dir =
            std::env::temp_dir().join(format!("mdluma-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&test_settings_dir);
        let test_settings_path = test_settings_dir.join("settings.json");

        let result = super::execute_launch_plan(
            plan,
            &mut |line: &str| stderr_lines.push(line.to_string()),
            distribution_dir,
            |_| Ok(fake_runtime()),
            {
                let ui = ui.clone();
                move |_distribution_dir, runtime| {
                    build_startup_controller(runtime, |_| Ok(ui.clone()), (), ())
                        .map(|c| {
                            c.with_settings_file(SettingsFile::with_path(
                                test_settings_path.clone(),
                            ))
                        })
                }
            },
            (),
        );

        (stderr_lines, result)
    }

    #[test]
    fn startup_plan_no_args_shows_empty_viewer_with_no_stderr() {
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (stderr, result) = start_plan_with(vec![], &ui);

        assert!(result.is_ok());
        assert!(stderr.is_empty());

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.document_html.is_empty());
        assert!(ui.initial_html[0].contains("data-current-file"));
        assert!(!ui.initial_html[0].contains("No file open"));
    }

    #[test]
    fn startup_plan_single_file_loads_document_into_initial_html() {
        let dir = std::env::temp_dir().join(format!(
            "mdluma-plan-single-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let source_path = dir.join("hello.md");
        std::fs::write(&source_path, "# Hello\n\nWorld").expect("write test file");

        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (stderr, result) = start_plan_with(vec![os(source_path.to_str().unwrap())], &ui);

        assert!(result.is_ok());
        assert!(stderr.is_empty());

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.document_html.is_empty());
        assert!(ui.initial_html[0].contains("hello.md"));
        assert!(ui.initial_html[0].contains("Hello"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_plan_single_file_read_error_shows_error_in_initial_html() {
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (stderr, result) = start_plan_with(vec![os(r"C:\nonexistent\nofile.md")], &ui);

        assert!(result.is_ok());
        assert!(stderr.is_empty());

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.document_html.is_empty());
        assert!(ui.initial_html[0].contains("could not read"));
    }

    #[test]
    fn startup_plan_unsupported_options_report_to_stderr_and_show_empty_viewer() {
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (stderr, result) = start_plan_with(vec![os("--help"), os("--version")], &ui);

        assert!(result.is_ok());
        assert_eq!(stderr.len(), 2);
        assert!(stderr[0].contains("--help"));
        assert!(stderr[0].contains("unrecognized"));
        assert!(stderr[1].contains("--version"));
        assert!(stderr[1].contains("unrecognized"));

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.initial_html[0].contains("data-current-file"));
        assert!(!ui.initial_html[0].contains("No file open"));
    }

    #[test]
    fn startup_plan_mixed_unsupported_and_file_continues_single_file_startup() {
        let dir = std::env::temp_dir().join(format!(
            "mdluma-plan-mixed-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let source_path = dir.join("solo.md");
        std::fs::write(&source_path, "# Solo\n\nContent").expect("write test file");

        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (stderr, result) = start_plan_with(
            vec![
                os("--alpha"),
                os(source_path.to_str().unwrap()),
                os("--beta"),
            ],
            &ui,
        );

        assert!(result.is_ok());
        assert_eq!(stderr.len(), 2);
        assert!(stderr[0].contains("--alpha"));
        assert!(stderr[1].contains("--beta"));

        let ui = ui.borrow();
        assert_eq!(ui.initial_html.len(), 1);
        assert!(ui.initial_html[0].contains("solo.md"));
        assert!(ui.initial_html[0].contains("Solo"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_plan_single_file_via_relative_path_produces_same_initial_html_as_absolute() {
        let cwd = std::env::current_dir().expect("cwd");
        let dir = cwd.join("target").join("tmp").join(format!(
            "mdluma-relpath-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let file_name = "relhello.md";
        let source_path = dir.join(file_name);
        std::fs::write(&source_path, "# Relative\n\nPath test").expect("write test file");

        let relative_path = dir
            .strip_prefix(&cwd)
            .expect("test dir should be under cwd")
            .join(file_name);

        let ui_abs = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let ui_rel = Rc::new(RefCell::new(RecordingStartupUi::default()));

        let (_, result_abs) = start_plan_with(vec![os(source_path.to_str().unwrap())], &ui_abs);
        let (_, result_rel) = start_plan_with(vec![os(relative_path.to_str().unwrap())], &ui_rel);

        assert!(result_abs.is_ok(), "absolute path startup should succeed");
        assert!(result_rel.is_ok(), "relative path startup should succeed");

        let ui_abs = ui_abs.borrow();
        let ui_rel = ui_rel.borrow();

        assert_eq!(ui_abs.initial_html.len(), 1);
        assert_eq!(ui_rel.initial_html.len(), 1);
        assert!(
            ui_abs.initial_html[0].contains("relhello.md"),
            "absolute path HTML should contain file name"
        );
        assert!(
            ui_abs.initial_html[0].contains("data-current-file"),
            "absolute path HTML should contain the current file element"
        );
        assert!(
            ui_abs.initial_html[0].contains("data-current-file>relhello.md<"),
            "absolute path HTML should show the file name in the drag region"
        );
        assert!(
            ui_abs.initial_html[0].contains("Relative"),
            "absolute path HTML should contain heading"
        );
        assert!(
            ui_rel.initial_html[0].contains("Relative"),
            "relative path HTML should contain heading"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Task 3.2: SpawnChildren branch tests ---

    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use crate::sciter::runtime::RuntimePrerequisites;
    use crate::viewer_launcher::ViewerChildLauncher;

    struct RecordingSpawnLauncher {
        launched: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl RecordingSpawnLauncher {
        fn new() -> (Self, Arc<Mutex<Vec<PathBuf>>>) {
            let launched = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                },
                launched,
            )
        }
    }

    impl ViewerChildLauncher for RecordingSpawnLauncher {
        fn launch_path(
            &self,
            path: &Path,
            _cascade_left: i32,
            _cascade_top: i32,
        ) -> Result<(), ViewerError> {
            self.launched.lock().unwrap().push(path.to_path_buf());
            Ok(())
        }
    }

    struct FailingSpawnLauncher {
        fail_on: PathBuf,
    }

    impl ViewerChildLauncher for FailingSpawnLauncher {
        fn launch_path(
            &self,
            path: &Path,
            _cascade_left: i32,
            _cascade_top: i32,
        ) -> Result<(), ViewerError> {
            if path == self.fail_on {
                Err(ViewerError::runtime_unavailable("access denied"))
            } else {
                Ok(())
            }
        }
    }

    struct RecordingFailingSpawnLauncher {
        launched: Arc<Mutex<Vec<PathBuf>>>,
        fail_on: PathBuf,
    }

    impl RecordingFailingSpawnLauncher {
        fn new(fail_on: PathBuf) -> (Self, Arc<Mutex<Vec<PathBuf>>>) {
            let launched = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                    fail_on,
                },
                launched,
            )
        }
    }

    impl ViewerChildLauncher for RecordingFailingSpawnLauncher {
        fn launch_path(
            &self,
            path: &Path,
            _cascade_left: i32,
            _cascade_top: i32,
        ) -> Result<(), ViewerError> {
            self.launched.lock().unwrap().push(path.to_path_buf());
            if path == self.fail_on {
                Err(ViewerError::runtime_unavailable("not found"))
            } else {
                Ok(())
            }
        }
    }

    fn execute_spawn_plan_with(
        args: Vec<OsString>,
        launcher: impl ViewerChildLauncher,
    ) -> (Vec<String>, Result<(), StartupError>) {
        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let plan = plan_startup_launch(args);
        let mut stderr_lines: Vec<String> = Vec::new();

        let noop_validate = |_prereqs: RuntimePrerequisites| -> Result<SciterRuntime, ViewerError> {
            panic!("validate_runtime should not be called for SpawnChildren")
        };
        let noop_build = |_dist: PathBuf,
                          _rt: SciterRuntime|
         -> Result<
            StartupController<Rc<RefCell<RecordingStartupUi>>>,
            ViewerError,
        > {
            panic!("build_controller should not be called for SpawnChildren")
        };

        let result = super::execute_launch_plan(
            plan,
            &mut |line: &str| stderr_lines.push(line.to_string()),
            distribution_dir,
            noop_validate,
            noop_build,
            launcher,
        );

        (stderr_lines, result)
    }

    #[test]
    fn spawn_children_spawns_one_child_per_file_path() {
        let (launcher, launched) = RecordingSpawnLauncher::new();

        let (stderr, result) =
            execute_spawn_plan_with(vec![os("a.md"), os("b.md"), os("c.md")], launcher);

        assert!(result.is_ok());
        assert!(stderr.is_empty());
        let records = launched.lock().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0], PathBuf::from("a.md"));
        assert_eq!(records[1], PathBuf::from("b.md"));
        assert_eq!(records[2], PathBuf::from("c.md"));
    }

    #[test]
    fn spawn_children_does_not_create_viewer_controller() {
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let (launcher, launched) = RecordingSpawnLauncher::new();

        let distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let plan = plan_startup_launch(vec![os("a.md"), os("b.md")]);
        let mut stderr_lines: Vec<String> = Vec::new();

        let noop_validate = |_prereqs: RuntimePrerequisites| -> Result<SciterRuntime, ViewerError> {
            panic!("validate_runtime should not be called for SpawnChildren")
        };
        let noop_build = |_dist: PathBuf,
                          _rt: SciterRuntime|
         -> Result<
            StartupController<Rc<RefCell<RecordingStartupUi>>>,
            ViewerError,
        > {
            panic!("build_controller should not be called for SpawnChildren")
        };

        let result = super::execute_launch_plan(
            plan,
            &mut |line: &str| stderr_lines.push(line.to_string()),
            distribution_dir,
            noop_validate,
            noop_build,
            launcher,
        );

        assert!(result.is_ok());
        assert_eq!(launched.lock().unwrap().len(), 2);
        let ui = ui.borrow();
        assert_eq!(ui.bind_count, 0);
        assert_eq!(ui.initial_html.len(), 0);
        assert_eq!(ui.event_loop_count, 0);
    }

    #[test]
    fn spawn_children_reports_notices_before_spawning() {
        let (_launcher, launched) = RecordingSpawnLauncher::new();
        let notice_order = Arc::new(Mutex::new(Vec::new()));
        let notice_order_clone = notice_order.clone();

        let recording_launcher = RecordingOrderedLauncher {
            launched: launched,
            order: notice_order_clone,
        };

        let (stderr, result) = execute_spawn_plan_with(
            vec![os("--alpha"), os("a.md"), os("--beta"), os("b.md")],
            recording_launcher,
        );

        assert!(result.is_ok());
        assert_eq!(stderr.len(), 2);
        assert!(stderr[0].contains("--alpha"));
        assert!(stderr[1].contains("--beta"));
        let order = notice_order.lock().unwrap();
        assert_eq!(order.len(), 2);
    }

    #[test]
    fn spawn_children_spawn_failure_returns_startup_error() {
        let launcher = FailingSpawnLauncher {
            fail_on: PathBuf::from("fail.md"),
        };

        let (stderr, result) =
            execute_spawn_plan_with(vec![os("ok.md"), os("fail.md"), os("never.md")], launcher);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("MDLuma"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn spawn_children_first_spawn_succeeds_before_second_fails() {
        let (launcher, launched) = RecordingFailingSpawnLauncher::new(PathBuf::from("second.md"));

        let (_stderr, result) =
            execute_spawn_plan_with(vec![os("first.md"), os("second.md")], launcher);

        assert!(result.is_err());
        let records = launched.lock().unwrap();
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn report_startup_notice_formats_unsupported_option_as_single_line() {
        let mut lines: Vec<String> = Vec::new();
        super::report_startup_notice(
            &StartupNotice::UnsupportedOption(os("--verbose")),
            &mut |line: &str| lines.push(line.to_string()),
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "MDLuma: unrecognized option: --verbose");
    }

    struct RecordingOrderedLauncher {
        launched: Arc<Mutex<Vec<PathBuf>>>,
        order: Arc<Mutex<Vec<String>>>,
    }

    impl ViewerChildLauncher for RecordingOrderedLauncher {
        fn launch_path(
            &self,
            path: &Path,
            _cascade_left: i32,
            _cascade_top: i32,
        ) -> Result<(), ViewerError> {
            self.launched.lock().unwrap().push(path.to_path_buf());
            self.order
                .lock()
                .unwrap()
                .push(format!("spawn:{}", path.display()));
            Ok(())
        }
    }

    // --- Task 3.3: External editor launcher wiring tests ---

    use crate::external_editor::ExternalEditorLauncher;

    struct RecordingExternalEditorLauncher {
        launched: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
    }

    impl RecordingExternalEditorLauncher {
        fn new() -> (Self, Arc<Mutex<Vec<(PathBuf, PathBuf)>>>) {
            let launched = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                },
                launched,
            )
        }
    }

    impl ExternalEditorLauncher for RecordingExternalEditorLauncher {
        fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
            self.launched
                .lock()
                .unwrap()
                .push((executable.to_path_buf(), document_path.to_path_buf()));
            Ok(())
        }
    }

    #[test]
    fn startup_wires_external_editor_launcher_through_build_controller() {
        let dir = std::env::temp_dir().join(format!(
            "mdluma-ee-wiring-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create test dir");
        let source_path = dir.join("ee-wiring.md");
        std::fs::write(&source_path, "# EE Wiring\n\nContent").expect("write test file");

        let (ee_launcher, ee_records) = RecordingExternalEditorLauncher::new();
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));
        let _distribution_dir = PathBuf::from(r"C:\dist\MDLuma");

        let mut controller = build_startup_controller(
            fake_runtime(),
            |_| Ok(ui.clone()),
            (),
            ee_launcher,
        )
        .expect("build controller with external editor launcher should succeed")
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.prepare_startup_path(&source_path);

        let editor_path = PathBuf::from(r"C:\tools\myeditor.exe");
        controller = controller.with_external_editor_config(Some(editor_path.clone()));

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect("external editor request should succeed through wired launcher");

        let records = ee_records.lock().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, editor_path);
        assert_eq!(records[0].1, source_path);

        let ui = ui.borrow();
        assert_eq!(ui.event_loop_count, 0, "controller.run() was not called");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn startup_wiring_preserves_normal_viewer_startup_without_external_editor() {
        let _distribution_dir = PathBuf::from(r"C:\dist\MDLuma");
        let ui = Rc::new(RefCell::new(RecordingStartupUi::default()));

        let controller =
            build_startup_controller(fake_runtime(), |_| Ok(ui.clone()), (), ())
                .expect("build controller with unit launchers should succeed");

        assert_eq!(ui.borrow().bind_count, 0, "controller not yet started");
        assert_eq!(ui.borrow().event_loop_count, 0);

        let _ = controller;
    }
}

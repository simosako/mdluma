pub const APP_NAME: &str = "MDLuma";

use crate::document::DocumentLoader;
use crate::errors::ViewerError;
use crate::external_editor::ExternalEditorLauncher;
use crate::html_shell::{HtmlShell, ShellModel};
use crate::markdown::MarkdownRenderer;
use crate::open_paths::plan_drop_open;
use crate::platform::{FileDialog, FontDialog, FontDialogResult, OpenFileResult};
use crate::sciter::window::{ViewerCommand, ViewerCommandBinder, ViewerCommandHandler, ViewerUi};
use crate::settings::{
    BodyFontSettings, Settings, SettingsFile, ThemePreference, WindowGeometry,
    DEFAULT_CONTENT_MAX_WIDTH_PX,
};
use crate::ui::Theme;
use crate::viewer_launcher::ViewerChildLauncher;
use std::path::{Path, PathBuf};

fn report_viewer_error(error: &ViewerError) {
    crate::debug_log!("viewer diagnostic: {}", error.operator_diagnostic());
    #[cfg(not(debug_assertions))]
    let _ = error;
}

struct LoadedSettings {
    theme: Theme,
    body_font: Option<BodyFontSettings>,
    external_editor: Option<PathBuf>,
    recent_files: Vec<PathBuf>,
    settings_file: SettingsFile,
    window_geometry: Option<WindowGeometry>,
    content_max_width_px: u16,
}

impl LoadedSettings {
    fn from_file(settings_file: SettingsFile) -> Self {
        let settings = settings_file.load();
        Self {
            theme: Theme::from(settings.theme),
            body_font: settings.body_font,
            external_editor: settings.external_editor,
            recent_files: settings.recent_files,
            settings_file,
            window_geometry: settings.window_geometry,
            content_max_width_px: settings.content_max_width_px,
        }
    }
}

#[cfg(test)]
fn create_test_settings_file() -> SettingsFile {
    let dir = std::env::temp_dir().join(format!("mdluma-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    SettingsFile::with_path(dir.join("settings.json"))
}

pub struct AppController<D, F, L, R, H, U, S = (), E = ()> {
    dialog: D,
    font_dialog: F,
    loader: L,
    renderer: R,
    shell: H,
    ui: U,
    launcher: S,
    external_editor_launcher: E,
    state: ViewerState,
    theme: Theme,
    body_font: Option<BodyFontSettings>,
    external_editor: Option<PathBuf>,
    recent_files: Vec<PathBuf>,
    settings_file: SettingsFile,
    window_geometry: Option<WindowGeometry>,
    content_max_width_px: u16,
}

#[cfg(test)]
impl<D, L, R, H, U> AppController<D, (), L, R, H, U, ()>
where
    D: FileDialog,
    L: DocumentLoader,
    R: MarkdownRenderer,
    H: HtmlShell,
    U: ViewerUi,
{
    pub fn new(dialog: D, loader: L, renderer: R, shell: H, ui: U) -> Self {
        let s = LoadedSettings::from_file(create_test_settings_file());
        Self {
            dialog,
            font_dialog: (),
            loader,
            renderer,
            shell,
            ui,
            launcher: (),
            external_editor_launcher: (),
            state: ViewerState::NoDocument,
            theme: s.theme,
            body_font: s.body_font,
            external_editor: s.external_editor,
            recent_files: s.recent_files,
            settings_file: s.settings_file,
            window_geometry: s.window_geometry,
            content_max_width_px: s.content_max_width_px,
        }
    }

    pub fn with_state(
        dialog: D,
        loader: L,
        renderer: R,
        shell: H,
        ui: U,
        state: ViewerState,
    ) -> Self {
        let s = LoadedSettings::from_file(create_test_settings_file());
        Self {
            dialog,
            font_dialog: (),
            loader,
            renderer,
            shell,
            ui,
            launcher: (),
            external_editor_launcher: (),
            state,
            theme: s.theme,
            body_font: s.body_font,
            external_editor: s.external_editor,
            recent_files: s.recent_files,
            settings_file: s.settings_file,
            window_geometry: s.window_geometry,
            content_max_width_px: s.content_max_width_px,
        }
    }
}

impl<D, F, L, R, H, U, S, E> AppController<D, F, L, R, H, U, S, E>
where
    D: FileDialog,
    F: FontDialog,
    L: DocumentLoader,
    R: MarkdownRenderer,
    H: HtmlShell,
    U: ViewerUi,
    S: ViewerChildLauncher,
{
    #[cfg(test)]
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    #[cfg(test)]
    pub fn with_body_font(mut self, body_font: Option<BodyFontSettings>) -> Self {
        self.body_font = body_font;
        self
    }

    #[cfg(test)]
    pub fn with_settings_file(mut self, settings_file: SettingsFile) -> Self {
        let s = LoadedSettings::from_file(settings_file);
        self.theme = s.theme;
        self.body_font = s.body_font;
        self.external_editor = s.external_editor;
        self.recent_files = s.recent_files;
        self.window_geometry = s.window_geometry;
        self.content_max_width_px = s.content_max_width_px;
        self.settings_file = s.settings_file;
        self
    }

    fn build_settings(&self) -> Settings {
        Settings {
            theme: ThemePreference::from(self.theme),
            body_font: self.body_font.clone(),
            external_editor: self.external_editor.clone(),
            recent_files: self.recent_files.clone(),
            window_geometry: self.window_geometry.clone(),
            content_max_width_px: self.content_max_width_px,
        }
    }

    fn save_settings(&self) {
        let _ = self.settings_file.save(&self.build_settings());
    }

    pub fn start(&mut self) -> Result<(), ViewerError> {
        let html = self.render_state_html(&self.state)?;
        self.ui.show_initial(&html)
    }

    pub fn open_file_requested(&mut self) -> Result<(), ViewerError> {
        let owner = self.ui.native_window_handle();
        match self.dialog.pick_markdown_file(owner)? {
            OpenFileResult::Cancelled => Ok(()),
            OpenFileResult::Selected(path) => self.open_selected_path(&path),
        }
    }

    pub fn open_external_editor_setting(&mut self) -> Result<(), ViewerError> {
        let owner = self.ui.native_window_handle();
        match self.dialog.pick_external_editor_file(owner)? {
            OpenFileResult::Cancelled => Ok(()),
            OpenFileResult::Selected(path) => {
                self.external_editor = Some(path.clone());
                if let Err(error) = self.settings_file.save(&self.build_settings()) {
                    return self.show_error_state(error);
                }
                Ok(())
            }
        }
    }

    pub fn handle_viewer_command(&mut self, command: ViewerCommand) -> Result<(), ViewerError> {
        match command {
            ViewerCommand::OpenFileRequested => self.open_file_requested(),
            ViewerCommand::OpenDroppedFiles(paths) => self.open_dropped_files(paths),
            ViewerCommand::OpenRecentFile(index) => {
                if let Some(path) = self.recent_files.get(index) {
                    let path = path.clone();
                    self.open_selected_path(&path)
                } else {
                    Ok(())
                }
            }
            ViewerCommand::ErrorDismissRequested => self.dismiss_error(),
            ViewerCommand::ThemeToggleRequested => self.toggle_theme(),
            ViewerCommand::FontSettingsRequested => self.open_body_font_dialog(),
            ViewerCommand::ExternalEditorRequested => Ok(()),
            ViewerCommand::ExternalEditorSettingRequested => self.open_external_editor_setting(),
            ViewerCommand::OpenExternalUrl(url) => {
                crate::platform::open_url_in_browser(&url);
                Ok(())
            }
        }
    }

    pub fn open_dropped_files(&mut self, dropped_paths: Vec<PathBuf>) -> Result<(), ViewerError> {
        let plan = plan_drop_open(dropped_paths);
        if let Some(current_path) = plan.current_path {
            self.open_selected_path(&current_path)?;
        }
        let (base_left, base_top) = self.parent_window_position();
        const CASCADE_STEP: i32 = 30;
        const MAX_OFFSET: i32 = 200;
        for (index, child_path) in plan.child_paths.iter().enumerate() {
            let offset = ((index as i32 + 1) * CASCADE_STEP).min(MAX_OFFSET);
            let _ = self
                .launcher
                .launch_path(child_path, base_left + offset, base_top + offset);
        }
        Ok(())
    }

    #[cfg(windows)]
    fn parent_window_position(&self) -> (i32, i32) {
        if let Some(hwnd) = self.ui.native_window_handle() {
            let mut rect = windows_sys::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let ok = unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::GetWindowRect(hwnd, &mut rect)
            };
            if ok != 0 {
                // create_window のデフォルト位置は (100,100) なので、差分を返す
                return (rect.left - 100, rect.top - 100);
            }
        }
        (0, 0)
    }

    #[cfg(not(windows))]
    fn parent_window_position(&self) -> (i32, i32) {
        (0, 0)
    }

    fn toggle_theme(&mut self) -> Result<(), ViewerError> {
        self.theme = self.theme.toggle();
        self.ui.apply_theme(self.theme)?;
        let _ = self.settings_file.save(&self.build_settings());
        Ok(())
    }

    fn open_body_font_dialog(&mut self) -> Result<(), ViewerError> {
        let owner = self.ui.native_window_handle();
        match self
            .font_dialog
            .choose_body_font(owner, self.body_font.as_ref())
        {
            Ok(FontDialogResult::Selected(font)) => {
                self.body_font = Some(font);
                let _ = self.settings_file.save(&self.build_settings());
                self.ui.apply_body_font(self.body_font.as_ref())?;
                Ok(())
            }
            Ok(FontDialogResult::Cancelled) => Ok(()),
            Err(_error) => {
                crate::debug_log!("font dialog error: {}", _error.operator_diagnostic());
                Ok(())
            }
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> &ViewerState {
        &self.state
    }

    #[cfg(test)]
    pub fn body_font(&self) -> &Option<BodyFontSettings> {
        &self.body_font
    }

    #[cfg(test)]
    pub fn external_editor(&self) -> &Option<PathBuf> {
        &self.external_editor
    }

    pub fn prepare_startup_path(&mut self, path: &std::path::Path) {
        let source = match self.loader.load(path) {
            Ok(source) => source,
            Err(error) => {
                report_viewer_error(&error);
                self.state = self.state.clone().with_error(error);
                return;
            }
        };

        let rendered = match self.renderer.render(&source) {
            Ok(rendered) => rendered,
            Err(error) => {
                report_viewer_error(&error);
                self.state = self.state.clone().with_error(error);
                return;
            }
        };

        self.state = ViewerState::document_loaded(rendered);
        self.record_recent_file(path);
    }

    fn open_selected_path(&mut self, path: &std::path::Path) -> Result<(), ViewerError> {
        let source = match self.loader.load(path) {
            Ok(source) => source,
            Err(error) => {
                report_viewer_error(&error);
                return self.show_error_state(error);
            }
        };

        let rendered = match self.renderer.render(&source) {
            Ok(rendered) => rendered,
            Err(error) => {
                report_viewer_error(&error);
                return self.show_error_state(error);
            }
        };
        let next_state = ViewerState::document_loaded(rendered);
        self.record_recent_file(path);
        let html = match self.render_state_html(&next_state) {
            Ok(html) => html,
            Err(error) => {
                report_viewer_error(&error);
                return self.show_error_state(error);
            }
        };

        self.ui.show_document(&html)?;
        self.state = next_state;
        Ok(())
    }

    fn record_recent_file(&mut self, path: &std::path::Path) {
        let canonical = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.recent_files.retain(|p| p != &canonical);
        self.recent_files.insert(0, canonical);
        self.recent_files.truncate(10);
        let _ = self.settings_file.save(&self.build_settings());
    }

    fn render_state_html(&self, state: &ViewerState) -> Result<String, ViewerError> {
        let mut html = self.shell.render_shell(ShellModel {
            app_name: APP_NAME,
            state,
            theme: self.theme,
            body_font: self.body_font.as_ref(),
            recent_files: &self.recent_files,
        })?;

        if self.content_max_width_px != DEFAULT_CONTENT_MAX_WIDTH_PX {
            let replacement = format!("max-width: {}px;", self.content_max_width_px);
            let default_max_width = format!("max-width: {}px;", DEFAULT_CONTENT_MAX_WIDTH_PX);
            let replaced = html.replacen(&default_max_width, &replacement, 1);
            if replaced == html {
                crate::debug_log!(
                    "content max width override marker not found: expected '{default_max_width}'"
                );
            } else {
                html = replaced;
            }
        }

        Ok(html)
    }

    fn show_error_state(&mut self, error: ViewerError) -> Result<(), ViewerError> {
        let next_state = self.state.clone().with_error(error);
        match self.render_state_html(&next_state) {
            Ok(html) => self.ui.show_document(&html)?,
            Err(shell_error) => {
                report_viewer_error(&shell_error);
                self.ui.show_error(&shell_error)?;
            }
        }
        self.state = next_state;
        Ok(())
    }

    fn dismiss_error(&mut self) -> Result<(), ViewerError> {
        if !self.state.is_error_visible() {
            return Ok(());
        }

        let next_state = self.state.clone().dismiss_error();
        let html = match self.render_state_html(&next_state) {
            Ok(html) => html,
            Err(error) => {
                report_viewer_error(&error);
                return self.show_error_state(error);
            }
        };

        if next_state.is_no_document() {
            self.ui.show_initial(&html)?;
        } else {
            self.ui.show_document(&html)?;
        }
        self.state = next_state;
        Ok(())
    }
}

impl<D, F, L, R, H, U, S, E> AppController<D, F, L, R, H, U, S, E>
where
    D: FileDialog,
    F: FontDialog,
    L: DocumentLoader,
    R: MarkdownRenderer,
    H: HtmlShell,
    U: ViewerUi,
    S: ViewerChildLauncher,
    E: ExternalEditorLauncher,
{
    pub fn with_launchers(
        dialog: D,
        font_dialog: F,
        loader: L,
        renderer: R,
        shell: H,
        ui: U,
        launcher: S,
        external_editor_launcher: E,
    ) -> Self {
        let s = LoadedSettings::from_file(SettingsFile::new());
        Self {
            dialog,
            font_dialog,
            loader,
            renderer,
            shell,
            ui,
            launcher,
            external_editor_launcher,
            state: ViewerState::NoDocument,
            theme: s.theme,
            body_font: s.body_font,
            external_editor: s.external_editor,
            recent_files: s.recent_files,
            settings_file: s.settings_file,
            window_geometry: s.window_geometry,
            content_max_width_px: s.content_max_width_px,
        }
    }

    #[cfg(test)]
    pub fn with_external_editor_config(mut self, path: Option<PathBuf>) -> Self {
        self.external_editor = path;
        self
    }

    #[cfg(test)]
    pub fn set_state(mut self, state: ViewerState) -> Self {
        self.state = state;
        self
    }

    #[cfg(test)]
    pub fn with_external_editor_launcher_and_state(
        dialog: D,
        font_dialog: F,
        loader: L,
        renderer: R,
        shell: H,
        ui: U,
        child_launcher: S,
        external_editor_launcher: E,
        state: ViewerState,
    ) -> Self {
        let s = LoadedSettings::from_file(create_test_settings_file());
        Self {
            dialog,
            font_dialog,
            loader,
            renderer,
            shell,
            ui,
            launcher: child_launcher,
            external_editor_launcher,
            state,
            theme: s.theme,
            body_font: s.body_font,
            external_editor: s.external_editor,
            recent_files: s.recent_files,
            settings_file: s.settings_file,
            window_geometry: s.window_geometry,
            content_max_width_px: s.content_max_width_px,
        }
    }

    pub fn open_in_external_editor(&mut self) -> Result<(), ViewerError> {
        let Some(document) = self.state.current_document() else {
            return Ok(());
        };
        let executable = match &self.external_editor {
            Some(path) => path.as_path(),
            None => Path::new("notepad.exe"),
        };
        if let Err(error) = self
            .external_editor_launcher
            .launch(executable, &document.path)
        {
            report_viewer_error(&error);
            let _ = self.show_error_state(error.clone());
            return Err(error);
        }
        if let Err(error) = self.ui.request_close() {
            report_viewer_error(&error);
            let _ = self.show_error_state(error.clone());
            return Err(error);
        }
        Ok(())
    }

    pub fn run(mut self) -> Result<(), ViewerError>
    where
        U: Clone + ViewerCommandBinder,
    {
        let mut ui = self.ui.clone();
        ui.bind_viewer_command_handler(&mut self)?;
        self.start()?;
        let result = ui.run_event_loop();
        self.persist_captured_window_geometry();
        result
    }

    fn persist_captured_window_geometry(&mut self) {
        #[cfg(windows)]
        if let Some(geometry) = crate::sciter::ffi::take_captured_window_geometry() {
            self.window_geometry = Some(geometry);
            self.save_settings();
        }
    }
}

impl<D, F, L, R, H, U, S, E> ViewerCommandHandler for AppController<D, F, L, R, H, U, S, E>
where
    D: FileDialog,
    F: FontDialog,
    L: DocumentLoader,
    R: MarkdownRenderer,
    H: HtmlShell,
    U: ViewerUi,
    S: ViewerChildLauncher,
    E: ExternalEditorLauncher,
{
    fn handle_viewer_command(&mut self, command: ViewerCommand) -> Result<(), ViewerError> {
        match command {
            ViewerCommand::ExternalEditorRequested => self.open_in_external_editor(),
            other => AppController::handle_viewer_command(self, other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDocument {
    pub path: PathBuf,
    pub file_name: String,
    pub base_dir: PathBuf,
    pub html_body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerState {
    NoDocument,
    DocumentLoaded(RenderedDocument),
    ErrorVisible {
        previous: Option<Box<ViewerState>>,
        error: ViewerError,
    },
}

impl ViewerState {
    pub fn no_document() -> Self {
        Self::NoDocument
    }

    pub fn document_loaded(document: RenderedDocument) -> Self {
        Self::DocumentLoaded(document)
    }

    pub fn with_error(self, error: ViewerError) -> Self {
        let previous = match self {
            Self::ErrorVisible { previous, .. } => previous,
            state => Some(Box::new(state)),
        };

        Self::ErrorVisible { previous, error }
    }

    pub fn dismiss_error(self) -> Self {
        match self {
            Self::ErrorVisible { previous, .. } => {
                previous.map(|state| *state).unwrap_or(Self::NoDocument)
            }
            state => state,
        }
    }

    pub fn is_no_document(&self) -> bool {
        matches!(self, Self::NoDocument)
    }

    pub fn is_error_visible(&self) -> bool {
        matches!(self, Self::ErrorVisible { .. })
    }

    pub fn current_document(&self) -> Option<&RenderedDocument> {
        match self {
            Self::NoDocument => None,
            Self::DocumentLoaded(document) => Some(document),
            Self::ErrorVisible { previous, .. } => {
                previous.as_deref().and_then(ViewerState::current_document)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppController;
    use crate::document::{DocumentLoader, SourceDocument};
    use crate::errors::ViewerError;
use crate::html_shell::{HtmlShell, ShellModel};
    use crate::markdown::MarkdownRenderer;
    use crate::platform::{FileDialog, FontDialog, FontDialogResult, OpenFileResult};
    use crate::sciter::window::{ViewerCommand, ViewerUi};
    use crate::settings::{
        BodyFontSettings, Settings, SettingsFile, ThemePreference, DEFAULT_CONTENT_MAX_WIDTH_PX,
    };
    use crate::{RenderedDocument, Theme, ViewerState, APP_NAME};
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn viewer_state_represents_only_empty_loaded_or_error_visible() {
        let empty = ViewerState::no_document();
        assert!(empty.is_no_document());
        assert_eq!(empty.current_document(), None);

        let loaded = ViewerState::document_loaded(rendered_document("one.md"));
        assert_eq!(
            loaded
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("one.md")
        );

        let replaced = ViewerState::document_loaded(rendered_document("two.md"));
        assert_eq!(
            replaced
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("two.md")
        );
        assert!(replaced.current_document().is_some());

        let error = replaced.with_error(ViewerError::markdown_render("renderer stopped"));
        assert!(error.is_error_visible());
        assert!(error.current_document().is_some());
    }

    #[test]
    fn viewer_state_dismiss_error_restores_previous_state() {
        let loaded = ViewerState::document_loaded(rendered_document("one.md"));
        let restored = loaded
            .clone()
            .with_error(ViewerError::file_read("missing.md", "not found"))
            .dismiss_error();

        assert_eq!(restored, loaded);
    }

    #[test]
    fn viewer_error_variants_have_user_facing_messages() {
        let cases = [
            ViewerError::file_read(Path::new("missing.md"), "access denied").user_message(),
            ViewerError::invalid_encoding(Path::new("bad.md")).user_message(),
            ViewerError::markdown_render("parser failed").user_message(),
            ViewerError::runtime_missing(Path::new("sciter.dll")).user_message(),
            ViewerError::ui("SciterLoadHtml failed").user_message(),
        ];

        for message in cases {
            assert!(message.contains("MDLuma"));
            assert!(!message.trim().is_empty());
        }
    }

    #[test]
    fn viewer_state_holds_consistent_absolutized_path_and_base_dir_after_open() {
        let selected_path = PathBuf::from(r"C:\docs\guide.md");
        let absolutized_path = PathBuf::from(r"C:\docs\guide.md");
        let source = SourceDocument {
            path: absolutized_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let rendered = RenderedDocument {
            path: absolutized_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>document</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(selected_path),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui,
        );

        controller
            .open_file_requested()
            .expect("open should succeed");

        let doc = controller
            .state()
            .current_document()
            .expect("document loaded");
        assert_eq!(doc.path, absolutized_path);
        assert_eq!(doc.base_dir, absolutized_path.parent().unwrap());
        assert_eq!(doc.file_name, "guide.md");
        assert_eq!(
            doc.base_dir,
            doc.path.parent().unwrap(),
            "base_dir must be parent of path"
        );
    }

    #[test]
    fn viewer_state_holds_consistent_path_and_base_dir_after_prepare_startup() {
        let file_path = PathBuf::from(r"C:\docs\readme.md");
        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "readme.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Readme".to_string(),
        };
        let rendered = RenderedDocument {
            path: file_path.clone(),
            file_name: "readme.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Readme</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(Vec::new());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui,
        );

        controller.prepare_startup_path(&file_path);

        let doc = controller
            .state()
            .current_document()
            .expect("document loaded after prepare_startup_path");
        assert_eq!(doc.path, file_path);
        assert_eq!(doc.base_dir, file_path.parent().unwrap());
        assert_eq!(doc.file_name, "readme.md");
    }



    #[test]
    fn file_name_is_leaf_name_regardless_of_path_absolutization() {
        let selected_path = PathBuf::from(r"docs\notes.md");
        let absolutized = PathBuf::from(r"C:\work\docs\notes.md");
        let source = SourceDocument {
            path: absolutized.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\work\docs"),
            markdown: "# Notes".to_string(),
        };
        let rendered = RenderedDocument {
            path: absolutized,
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\work\docs"),
            html_body: "<h1>Notes</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>notes</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(selected_path),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui,
        );

        controller
            .open_file_requested()
            .expect("open should succeed");

        let doc = controller
            .state()
            .current_document()
            .expect("document loaded");
        assert_eq!(doc.file_name, "notes.md");
        assert!(
            !doc.file_name.contains('\\') && !doc.file_name.contains('/'),
            "file_name must be a leaf name without path separators"
        );
    }



    #[test]
    fn start_renders_initial_state_into_viewer_ui() {
        let shell = RecordingHtmlShell::new(vec![Ok("<html>initial</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
        );

        controller.start().expect("render initial state");

        assert_eq!(controller.state(), &ViewerState::NoDocument);
        assert_eq!(shell.recorded_state_count(), 1);
        assert!(shell.saw_no_document_state());
        assert_eq!(ui.initial_html(), vec!["<html>initial</html>".to_string()]);
        assert!(ui.document_html().is_empty());
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn open_request_success_loads_renders_updates_ui_and_state() {
        let selected_path = PathBuf::from(r"C:\docs\guide.md");
        let source = SourceDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let rendered = RenderedDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>document</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(selected_path.clone()),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered.clone())]),
            shell.clone(),
            ui.clone(),
        );

        controller
            .open_file_requested()
            .expect("open flow should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("guide.md")
        );
        assert!(controller.state().current_document().is_some());
        assert_eq!(
            shell.recorded_file_names(),
            vec![Some("guide.md".to_string())]
        );
        assert_eq!(
            shell.recorded_html_bodies(),
            vec![Some("<h1>Guide</h1>".to_string())]
        );
        assert_eq!(
            ui.document_html(),
            vec!["<html>document</html>".to_string()]
        );
        assert!(ui.initial_html().is_empty());
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn large_markdown_open_flow_keeps_single_document_state_and_local_only_output() {
        let dir = unique_test_dir("large-markdown-single-document");
        fs::create_dir_all(&dir).expect("create test dir");
        let source_path = dir.join("large.md");
        let markdown = large_markdown_fixture();
        fs::write(&source_path, &markdown).expect("write large markdown fixture");

        let shell = crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(source_path),
            crate::document::FileDocumentLoader,
            crate::markdown::ComrakMarkdownRenderer,
            shell,
            ui.clone(),
        );

        controller.start().expect("show initial shell");
        controller
            .open_file_requested()
            .expect("large markdown open flow should succeed");

        assert!(controller.state().current_document().is_some());
        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("large.md")
        );

        let rendered_html = ui
            .last_document_html()
            .expect("record rendered document html");
        assert!(rendered_html.contains("Section 0"));
        assert!(rendered_html.contains("Section 255"));
        assert!(rendered_html.contains("href=\"#\""));
        assert!(rendered_html.contains("data-href=\"https://example.com/remote\""));
        assert!(!rendered_html.contains("<a href=\"https://"));
        assert!(!rendered_html.contains("https://example.com/image.png"));
        assert_eq!(ui.document_html().len(), 1);
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn markdown_open_flow_sanitizes_unsafe_html_but_keeps_safe_semantic_markup() {
        let dir = unique_test_dir("markdown-sanitization-boundary");
        fs::create_dir_all(&dir).expect("create test dir");
        let source_path = dir.join("unsafe.md");
        fs::write(&source_path, markdown_sanitization_fixture())
            .expect("write markdown sanitization fixture");

        let shell = crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(source_path),
            crate::document::FileDocumentLoader,
            crate::markdown::ComrakMarkdownRenderer,
            shell,
            ui.clone(),
        );

        controller.start().expect("show initial shell");
        controller
            .open_file_requested()
            .expect("unsafe markdown open flow should succeed");

        let rendered_html = ui
            .last_document_html()
            .expect("record rendered document html");
        assert!(rendered_html.contains("<details open>"));
        assert!(rendered_html.contains("<kbd>Ctrl</kbd>"));
        assert!(rendered_html.contains("href=\"#\""));
        assert!(!rendered_html.to_ascii_lowercase().contains("onclick="));
        assert!(!rendered_html.contains("<iframe"));
        assert!(!rendered_html.contains("javascript:alert(1)"));
        assert!(!rendered_html.contains("file:///C:/docs/guide.md"));
        assert!(!rendered_html.contains("data:text/html"));
    }

    #[test]
    fn real_file_loader_relative_path_absolutizes_and_shows_leaf_name_only() {
        let cwd = std::env::current_dir().expect("current dir");
        let test_root = cwd.join("target").join("tmp-integ-path-norm");
        fs::create_dir_all(&test_root).expect("create test root");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let subdir = test_root.join(format!("subdir-{nonce}"));
        fs::create_dir_all(&subdir).expect("create subdir");
        let file_path = subdir.join("notes.md");
        fs::write(&file_path, "# Notes\n\nRelative path content.").expect("write markdown");

        let relative_path = file_path
            .strip_prefix(&cwd)
            .expect("path under cwd")
            .to_path_buf();
        let expected_absolute = std::path::absolute(&relative_path).expect("absolute");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(relative_path),
            crate::document::FileDocumentLoader,
            crate::markdown::ComrakMarkdownRenderer,
            shell,
            ui.clone(),
        );

        controller.start().expect("show initial shell");
        controller
            .open_file_requested()
            .expect("relative path open should succeed");

        let doc = controller
            .state()
            .current_document()
            .expect("document loaded");
        assert!(doc.path.is_absolute(), "path must be absolute");
        assert_eq!(doc.path, expected_absolute);
        assert_eq!(
            doc.base_dir,
            expected_absolute.parent().expect("parent"),
            "base_dir must be parent of absolutized path"
        );
        assert_eq!(doc.file_name, "notes.md");
        assert!(
            !doc.file_name.contains('\\') && !doc.file_name.contains('/'),
            "file_name must be leaf only, no path separators"
        );

        let rendered_html = ui.last_document_html().expect("rendered html");
        assert!(
            rendered_html.contains("notes.md"),
            "HTML must contain leaf file name"
        );
        let absolute_str = expected_absolute.to_str().expect("absolute path as str");
        assert!(
            !rendered_html.contains(absolute_str),
            "HTML must NOT contain full absolute path"
        );
        assert!(
            rendered_html.contains("data-current-file>notes.md<"),
            "HTML current file label must show leaf file name only"
        );

        let _ = fs::remove_dir_all(&subdir);
    }

    #[test]
    fn second_open_replaces_existing_document_and_keeps_single_document_state() {
        let first_path = PathBuf::from(r"C:\docs\guide.md");
        let second_path = PathBuf::from(r"C:\docs\notes.md");
        let first_source = SourceDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let second_source = SourceDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Notes".to_string(),
        };
        let first_rendered = RenderedDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let second_rendered = RenderedDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Notes</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![
            Ok("<html>guide</html>".to_string()),
            Ok("<html>notes</html>".to_string()),
        ]);
        let ui = RecordingViewerUi::default();
        let dialog = StubFileDialog::sequence(vec![
            OpenFileResult::Selected(first_path),
            OpenFileResult::Selected(second_path),
        ]);
        let loader = StubDocumentLoader::new(vec![Ok(first_source), Ok(second_source)]);
        let renderer =
            StubMarkdownRenderer::new(vec![Ok(first_rendered), Ok(second_rendered.clone())]);
        let mut controller =
            AppController::new(dialog, loader, renderer, shell.clone(), ui.clone());

        controller
            .open_file_requested()
            .expect("first open should succeed");
        controller
            .open_file_requested()
            .expect("second open should replace the current document");

        assert!(controller.state().current_document().is_some());
        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("notes.md")
        );
        assert_eq!(
            shell.recorded_file_names(),
            vec![Some("guide.md".to_string()), Some("notes.md".to_string())]
        );
        assert_eq!(
            ui.document_html(),
            vec![
                "<html>guide</html>".to_string(),
                "<html>notes</html>".to_string()
            ]
        );
    }

    #[test]
    fn second_open_replaces_previous_ui_local_selection_and_copy_status_artifacts() {
        const STALE_COPY_STATUS: &str = "stale-copy-status-from-first-document";
        const STALE_SELECTION_MARKER: &str = "data-stale-selection=\"guide-selection\"";
        const FRESH_COPY_STATUS: &str =
            "<section class=\"copy-status\" data-copy-status aria-live=\"polite\"></section>";

        let first_path = PathBuf::from(r"C:\docs\guide.md");
        let second_path = PathBuf::from(r"C:\docs\notes.md");
        let first_source = SourceDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let second_source = SourceDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Notes".to_string(),
        };
        let first_rendered = RenderedDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1><p>First document.</p>".to_string(),
        };
        let second_rendered = RenderedDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Notes</h1><p>Second document.</p>".to_string(),
        };
        let dialog = StubFileDialog::sequence(vec![
            OpenFileResult::Selected(first_path),
            OpenFileResult::Selected(second_path),
        ]);
        let loader = StubDocumentLoader::new(vec![Ok(first_source), Ok(second_source)]);
        let renderer =
            StubMarkdownRenderer::new(vec![Ok(first_rendered), Ok(second_rendered.clone())]);
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(dialog, loader, renderer, shell, ui.clone());

        controller
            .open_file_requested()
            .expect("first open should succeed");
        ui.inject_displayed_document_artifacts(STALE_COPY_STATUS, STALE_SELECTION_MARKER);

        let stale_document_html = ui
            .displayed_document_html()
            .expect("stale first document should be recorded");
        assert!(stale_document_html.contains(STALE_COPY_STATUS));
        assert!(stale_document_html.contains(STALE_SELECTION_MARKER));

        controller
            .open_file_requested()
            .expect("second open should replace the current document");

        let refreshed_document_html = ui
            .displayed_document_html()
            .expect("second document should replace the stale DOM");
        assert!(refreshed_document_html.contains("notes.md"));
        assert!(refreshed_document_html.contains("Second document."));
        assert!(refreshed_document_html.contains(FRESH_COPY_STATUS));
        assert!(refreshed_document_html.contains("<article class=\"markdown-body\""));
        assert!(!refreshed_document_html.contains("guide-selection"));
        assert!(!refreshed_document_html.contains(STALE_COPY_STATUS));
        assert!(!refreshed_document_html.contains(STALE_SELECTION_MARKER));

        // If show_document() stopped reloading the full HTML shell, the stale selection/copy
        // markers from the first document could remain and still be targetable for copy.
        assert_eq!(
            refreshed_document_html.matches(FRESH_COPY_STATUS).count(),
            1,
            "reloaded document should expose exactly one fresh copy-status anchor"
        );
        assert_eq!(
            refreshed_document_html
                .matches("<article class=\"markdown-body\" data-markdown-body")
                .count(),
            1,
            "reloaded document should expose exactly one fresh markdown-body anchor"
        );
    }

    #[test]
    fn open_request_cancel_preserves_current_state_without_ui_changes() {
        let existing = rendered_document("existing.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>unused</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
            ViewerState::document_loaded(existing.clone()),
        );

        controller
            .open_file_requested()
            .expect("cancel should preserve state");

        assert_eq!(controller.state().current_document(), Some(&existing));
        assert!(ui.initial_html().is_empty());
        assert!(ui.document_html().is_empty());
        assert!(ui.errors().is_empty());
        assert_eq!(shell.recorded_state_count(), 0);
    }

    #[test]
    fn open_request_read_failure_shows_error_and_preserves_previous_document() {
        let selected_path = PathBuf::from(r"C:\docs\missing.md");
        let existing = rendered_document("existing.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::selected(selected_path.clone()),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                &selected_path,
                "access denied",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
            ViewerState::document_loaded(existing.clone()),
        );

        controller
            .open_file_requested()
            .expect("read failure should stay in app flow");

        assert!(controller.state().is_error_visible());
        assert_eq!(controller.state().current_document(), Some(&existing));
        assert_eq!(
            shell.recorded_file_names(),
            vec![Some("existing.md".to_string())]
        );
        assert_eq!(ui.document_html(), vec!["<html>error</html>".to_string()]);
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn open_request_render_failure_shows_error_and_preserves_previous_document() {
        let selected_path = PathBuf::from(r"C:\docs\broken.md");
        let source = SourceDocument {
            path: selected_path.clone(),
            file_name: "broken.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Broken".to_string(),
        };
        let existing = rendered_document("existing.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::selected(selected_path),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Err(ViewerError::markdown_render("renderer stopped"))]),
            shell.clone(),
            ui.clone(),
            ViewerState::document_loaded(existing.clone()),
        );

        controller
            .open_file_requested()
            .expect("render failure should stay in app flow");

        assert!(controller.state().is_error_visible());
        assert_eq!(controller.state().current_document(), Some(&existing));
        assert_eq!(
            shell.recorded_file_names(),
            vec![Some("existing.md".to_string())]
        );
        assert_eq!(ui.document_html(), vec!["<html>error</html>".to_string()]);
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn dismiss_error_command_restores_previous_document_after_failed_open() {
        let selected_path = PathBuf::from(r"C:\docs\missing.md");
        let existing = rendered_document("existing.md");
        let shell = RecordingHtmlShell::new(vec![
            Ok("<html>error</html>".to_string()),
            Ok("<html>restored</html>".to_string()),
        ]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::selected(selected_path.clone()),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                &selected_path,
                "access denied",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
            ViewerState::document_loaded(existing.clone()),
        );

        controller
            .open_file_requested()
            .expect("read failure should stay in app flow");
        controller
            .handle_viewer_command(ViewerCommand::ErrorDismissRequested)
            .expect("dismiss should restore previous document");

        assert!(!controller.state().is_error_visible());
        assert_eq!(controller.state().current_document(), Some(&existing));
        assert_eq!(
            shell.recorded_file_names(),
            vec![
                Some("existing.md".to_string()),
                Some("existing.md".to_string())
            ]
        );
        assert_eq!(
            ui.document_html(),
            vec![
                "<html>error</html>".to_string(),
                "<html>restored</html>".to_string()
            ]
        );
    }

    #[test]
    fn theme_toggle_clears_document_overlay_error_state() {
        let existing = rendered_document("existing.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>themed</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            ViewerState::document_loaded(existing.clone()).with_error(ViewerError::file_read(
                Path::new(r"C:\docs\missing.md"),
                "missing",
            )),
        );

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("theme toggle should succeed");

        assert!(controller.state().is_error_visible());
        assert_eq!(controller.state().current_document(), Some(&existing));
        assert!(ui.initial_html().is_empty());
    }

    #[test]
    fn prepare_startup_path_success_sets_document_loaded_state_without_ui_effects() {
        let file_path = PathBuf::from(r"C:\docs\readme.md");
        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "readme.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Readme".to_string(),
        };
        let rendered = RenderedDocument {
            path: file_path.clone(),
            file_name: "readme.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Readme</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(Vec::new());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered.clone())]),
            shell,
            ui.clone(),
        );

        controller.prepare_startup_path(&file_path);

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("readme.md")
        );
        assert!(controller.state().current_document().is_some());
        assert!(!controller.state().is_error_visible());
        assert!(ui.initial_html().is_empty());
        assert!(ui.document_html().is_empty());
        assert!(ui.errors().is_empty());
    }



    #[test]
    fn prepare_startup_path_file_read_error_carries_variant_and_path_diagnostic() {
        let file_path = PathBuf::from(r"C:\restricted\secret.md");
        let shell = RecordingHtmlShell::new(Vec::new());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                &file_path,
                "permission denied",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui,
        );

        controller.prepare_startup_path(&file_path);

        let state = controller.state();
        assert!(state.is_error_visible());
        assert!(state.current_document().is_none());
        if let ViewerState::ErrorVisible { previous, error } = state {
            assert!(matches!(previous.as_deref(), Some(ViewerState::NoDocument)));
            assert_eq!(
                *error,
                ViewerError::FileRead {
                    path: file_path.clone(),
                    message: "permission denied".to_string(),
                }
            );
            assert!(error.user_message().contains("could not read"));
            assert!(error
                .operator_diagnostic()
                .contains(file_path.to_string_lossy().as_ref()));
        } else {
            panic!("expected ErrorVisible state with FileRead variant");
        }
    }

    #[test]
    fn prepare_startup_path_invalid_encoding_error_carries_variant_and_path_diagnostic() {
        let file_path = PathBuf::from(r"C:\data\binary.dat");
        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "binary.dat".to_string(),
            base_dir: PathBuf::from(r"C:\data"),
            markdown: "will not reach renderer".to_string(),
        };
        let shell = RecordingHtmlShell::new(Vec::new());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Err(ViewerError::invalid_encoding(&file_path))]),
            shell,
            ui,
        );

        controller.prepare_startup_path(&file_path);

        let state = controller.state();
        assert!(state.is_error_visible());
        assert!(state.current_document().is_none());
        if let ViewerState::ErrorVisible { previous, error } = state {
            assert!(matches!(previous.as_deref(), Some(ViewerState::NoDocument)));
            assert_eq!(
                *error,
                ViewerError::InvalidEncoding {
                    path: file_path.clone(),
                }
            );
            assert!(error.user_message().contains("UTF-8"));
            assert!(error
                .operator_diagnostic()
                .contains(file_path.to_string_lossy().as_ref()));
        } else {
            panic!("expected ErrorVisible state with InvalidEncoding variant");
        }
    }


    #[test]
    fn start_succeeds_after_prepare_startup_path_failure() {
        let file_path = PathBuf::from(r"C:\docs\missing.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>error-view</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(&file_path, "not found"))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        );

        controller.prepare_startup_path(&file_path);
        assert!(controller.state().is_error_visible());

        controller
            .start()
            .expect("start should succeed even after failed prepare");

        assert_eq!(
            ui.initial_html(),
            vec!["<html>error-view</html>".to_string()]
        );
        assert!(ui.document_html().is_empty());
        assert!(controller.state().is_error_visible());
        assert!(controller.state().current_document().is_none());
    }

    #[test]
    fn controller_dispatches_open_command_and_ignores_viewer_only_out_of_scope_commands() {
        let selected_path = PathBuf::from(r"C:\docs\guide.md");
        let source = SourceDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let rendered = RenderedDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>document</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let dialog = StubFileDialog::selected(selected_path.clone());
        let loader = StubDocumentLoader::new(vec![Ok(source)]);
        let renderer = StubMarkdownRenderer::new(vec![Ok(rendered)]);
        let mut controller =
            AppController::new(dialog.clone(), loader.clone(), renderer.clone(), shell, ui);

        controller
            .handle_viewer_command(ViewerCommand::OpenFileRequested)
            .expect("open command should dispatch to open flow");

        assert_eq!(dialog.pick_count(), 1);
        assert_eq!(loader.load_count(), 1);
        assert_eq!(renderer.render_count(), 1);
        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|document| document.file_name.as_str()),
            Some("guide.md")
        );

        let shell = RecordingHtmlShell::new(vec![Ok("<html>unused</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let dialog = StubFileDialog::selected(selected_path);
        let loader = StubDocumentLoader::new(vec![Ok(SourceDocument {
            path: PathBuf::from(r"C:\docs\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        })]);
        let renderer = StubMarkdownRenderer::new(vec![Ok(rendered_document("guide.md"))]);
        let controller =
            AppController::new(dialog.clone(), loader.clone(), renderer.clone(), shell, ui);

        assert_eq!(dialog.pick_count(), 0);
        assert_eq!(loader.load_count(), 0);
        assert_eq!(renderer.render_count(), 0);
        assert!(controller.state().is_no_document());
    }

    #[test]
    fn open_request_forwards_native_window_handle_to_file_dialog() {
        let selected_path = PathBuf::from(r"C:\docs\guide.md");
        let source = SourceDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let rendered = RenderedDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let owner = std::ptr::dangling_mut::<core::ffi::c_void>();
        let shell = RecordingHtmlShell::new(vec![Ok("<html>document</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        ui.set_native_window_handle(Some(owner));
        let dialog = StubFileDialog::selected(selected_path);
        let loader = StubDocumentLoader::new(vec![Ok(source)]);
        let renderer = StubMarkdownRenderer::new(vec![Ok(rendered)]);
        let mut controller =
            AppController::new(dialog.clone(), loader.clone(), renderer.clone(), shell, ui);

        controller
            .open_file_requested()
            .expect("open request should succeed");

        assert_eq!(dialog.last_owner(), Some(owner));
        assert_eq!(dialog.pick_count(), 1);
    }

    #[test]
    fn open_request_empty_path_load_failure_preserves_previous_document() {
        let empty_path = PathBuf::new();
        let existing = rendered_document("current.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::selected(empty_path),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                Path::new(""),
                "empty path rejected by resolve_document_path",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
            ViewerState::document_loaded(existing.clone()),
        );

        controller
            .open_file_requested()
            .expect("empty-path failure should stay in app flow");

        assert!(controller.state().is_error_visible());
        assert_eq!(
            controller.state().current_document(),
            Some(&existing),
            "empty path absolutization failure must not replace the current document (req 3.3)"
        );
        assert!(controller.state().current_document().is_some());
        assert_eq!(ui.document_html(), vec!["<html>error</html>".to_string()]);
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn prepare_startup_path_empty_path_load_failure_sets_error_visible_state() {
        let empty_path = PathBuf::new();
        let shell = RecordingHtmlShell::new(vec![Ok("<html>startup-error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                Path::new(""),
                "empty path rejected by resolve_document_path",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        );

        controller.prepare_startup_path(&empty_path);

        assert!(controller.state().is_error_visible());
        assert!(controller.state().current_document().is_none());

        controller
            .start()
            .expect("start should succeed after empty-path failure");

        assert_eq!(
            ui.initial_html(),
            vec!["<html>startup-error</html>".to_string()]
        );
        assert!(ui.document_html().is_empty());
        assert!(controller.state().is_error_visible());
    }

    #[test]
    fn drop_single_file_opens_in_current_window_via_existing_open_path() {
        let dir = unique_test_dir("drop-single-success");
        fs::create_dir_all(&dir).expect("create test dir");
        let file_path = dir.join("hello.md");
        fs::write(&file_path, "# Hello\n\nDrop").expect("write test file");

        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "hello.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# Hello\n\nDrop".to_string(),
        };
        let rendered = RenderedDocument {
            path: file_path.clone(),
            file_name: "hello.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>Hello</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>dropped</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller
            .open_dropped_files(vec![file_path])
            .expect("drop open should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("hello.md")
        );
        assert_eq!(ui.document_html(), vec!["<html>dropped</html>".to_string()]);
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_read_failure_shows_error_via_existing_error_flow() {
        let dir = unique_test_dir("drop-read-failure");
        fs::create_dir_all(&dir).expect("create test dir");
        let file_path = dir.join("bad.md");
        fs::write(&file_path, "ok").expect("write test file");

        let existing = rendered_document("before.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>drop-error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Err(ViewerError::file_read(
                &file_path,
                "access denied",
            ))]),
            StubMarkdownRenderer::new(Vec::new()),
            shell.clone(),
            ui.clone(),
            launcher,
            (),
        )
        .set_state(ViewerState::document_loaded(existing.clone()));

        controller
            .open_dropped_files(vec![file_path])
            .expect("drop read failure should stay in app flow");

        assert!(controller.state().is_error_visible());
        assert_eq!(controller.state().current_document(), Some(&existing));
        assert_eq!(
            ui.document_html(),
            vec!["<html>drop-error</html>".to_string()]
        );
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_encoding_failure_shows_error_via_existing_error_flow() {
        let dir = unique_test_dir("drop-encoding-failure");
        fs::create_dir_all(&dir).expect("create test dir");
        let file_path = dir.join("binary.dat");
        fs::write(&file_path, "raw").expect("write test file");

        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "binary.dat".to_string(),
            base_dir: dir.clone(),
            markdown: "raw".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>encoding-error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Err(ViewerError::invalid_encoding(&file_path))]),
            shell,
            ui.clone(),
            launcher,
            (),
        );

        controller
            .open_dropped_files(vec![file_path])
            .expect("drop encoding failure should stay in app flow");

        assert!(controller.state().is_error_visible());
        assert_eq!(
            ui.document_html(),
            vec!["<html>encoding-error</html>".to_string()]
        );
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_empty_paths_does_not_change_current_window() {
        let existing = rendered_document("current.md");
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            ui.clone(),
            launcher,
            (),
        )
        .set_state(ViewerState::document_loaded(existing.clone()));

        controller
            .open_dropped_files(vec![])
            .expect("empty drop should be ok");

        assert_eq!(controller.state().current_document(), Some(&existing));
        assert!(ui.document_html().is_empty());
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_folders_only_does_not_change_current_window() {
        let dir = unique_test_dir("drop-folders-only");
        fs::create_dir_all(dir.join("subdir")).expect("create subdirs");

        let existing = rendered_document("current.md");
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            ui.clone(),
            launcher,
            (),
        )
        .set_state(ViewerState::document_loaded(existing.clone()));

        controller
            .open_dropped_files(vec![dir.join("subdir")])
            .expect("folders-only drop should be ok");

        assert_eq!(controller.state().current_document(), Some(&existing));
        assert!(ui.document_html().is_empty());
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_replaces_existing_document_with_dropped_file() {
        let dir = unique_test_dir("drop-replaces-doc");
        fs::create_dir_all(&dir).expect("create test dir");
        let drop_path = dir.join("new.md");
        fs::write(&drop_path, "# New\n\nContent").expect("write test file");

        let existing = rendered_document("old.md");
        let source = SourceDocument {
            path: drop_path.clone(),
            file_name: "new.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# New\n\nContent".to_string(),
        };
        let rendered = RenderedDocument {
            path: drop_path.clone(),
            file_name: "new.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>New</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>replaced</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")))
        .set_state(ViewerState::document_loaded(existing));

        controller
            .open_dropped_files(vec![drop_path])
            .expect("drop replace should succeed");

        assert!(controller.state().current_document().is_some());
        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("new.md")
        );
        assert_eq!(
            ui.document_html(),
            vec!["<html>replaced</html>".to_string()]
        );
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_success_replaces_error_visible_state() {
        let dir = unique_test_dir("drop-replaces-error");
        fs::create_dir_all(&dir).expect("create test dir");
        let drop_path = dir.join("fixed.md");
        fs::write(&drop_path, "# Fixed").expect("write test file");

        let source = SourceDocument {
            path: drop_path.clone(),
            file_name: "fixed.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# Fixed".to_string(),
        };
        let rendered = RenderedDocument {
            path: drop_path.clone(),
            file_name: "fixed.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>Fixed</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>fixed-doc</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let error_state = ViewerState::no_document()
            .with_error(ViewerError::file_read("missing.md", "not found"));
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")))
        .set_state(error_state);

        controller
            .open_dropped_files(vec![drop_path])
            .expect("drop into error state should succeed");

        assert!(!controller.state().is_error_visible());
        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("fixed.md")
        );
        assert_eq!(
            ui.document_html(),
            vec!["<html>fixed-doc</html>".to_string()]
        );
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn drop_multiple_files_opens_first_in_current_and_launches_rest_as_children() {
        let dir = unique_test_dir("drop-multi-child");
        fs::create_dir_all(&dir).expect("create test dir");
        let f1 = dir.join("first.md");
        let f2 = dir.join("second.md");
        let f3 = dir.join("third.md");
        fs::write(&f1, "# First").expect("write f1");
        fs::write(&f2, "# Second").expect("write f2");
        fs::write(&f3, "# Third").expect("write f3");

        let source1 = SourceDocument {
            path: f1.clone(),
            file_name: "first.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# First".to_string(),
        };
        let rendered1 = RenderedDocument {
            path: f1.clone(),
            file_name: "first.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>First</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>first</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source1)]),
            StubMarkdownRenderer::new(vec![Ok(rendered1)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller
            .open_dropped_files(vec![f1.clone(), f2.clone(), f3.clone()])
            .expect("multi drop should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("first.md")
        );
        assert_eq!(ui.document_html(), vec!["<html>first</html>".to_string()]);
        let records = launched.borrow();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0], f2);
        assert_eq!(records[1], f3);
    }

    #[test]
    fn drop_command_dispatches_to_open_dropped_files() {
        let dir = unique_test_dir("drop-cmd-dispatch");
        fs::create_dir_all(&dir).expect("create test dir");
        let file_path = dir.join("cmd.md");
        fs::write(&file_path, "# Cmd").expect("write test file");

        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "cmd.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# Cmd".to_string(),
        };
        let rendered = RenderedDocument {
            path: file_path.clone(),
            file_name: "cmd.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>Cmd</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![Ok("<html>cmd-drop</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller
            .handle_viewer_command(ViewerCommand::OpenDroppedFiles(vec![file_path]))
            .expect("command dispatch should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("cmd.md")
        );
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn theme_toggle_requested_toggles_theme_and_saves_settings() {
        use crate::ui::Theme;
        let shell =
            RecordingHtmlShell::new(vec![Ok("<html theme=\"dark\">toggled</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_theme(Theme::Dark);

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("theme toggle should succeed");

        assert!(ui.initial_html().is_empty());
        assert!(ui.document_html().is_empty());
    }

    #[test]
    fn start_restores_dark_theme_from_settings_file() {
        let dir = unique_test_dir("theme-start-restore-dark");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        SettingsFile::with_path(settings_path)
            .save(&Settings {
                theme: ThemePreference::Dark,
                ..Settings::default()
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");

        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("theme=\"dark\""),
            "saved dark theme should be restored on startup"
        );
    }

    #[test]
    fn start_uses_light_theme_when_settings_file_is_missing() {
        let dir = unique_test_dir("theme-start-missing-settings");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");

        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("theme=\"light\""),
            "missing settings file should default to light theme"
        );
        assert!(!initial_html.contains("theme=\"dark\""));
    }

    #[test]
    fn start_applies_content_max_width_px_from_settings_file() {
        let dir = unique_test_dir("content-width-start-restore");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        SettingsFile::with_path(settings_path)
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: None,
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: 980,
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");

        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("max-width: 980px;"),
            "saved content_max_width_px should override default max-width"
        );
        assert!(
            !initial_html.contains("max-width: 1040px;"),
            "default max-width should be replaced when setting is present"
        );
    }

    #[test]
    fn theme_toggle_saves_updated_theme_to_settings_file() {
        let dir = unique_test_dir("theme-toggle-save-settings");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui,
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("theme toggle should succeed");

        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(saved.theme, ThemePreference::Dark);
    }

    #[test]
    fn repeated_theme_toggles_persist_the_latest_theme() {
        let dir = unique_test_dir("theme-toggle-repeated-save-settings");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui,
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("first toggle should succeed");
        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("second toggle should succeed");

        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(saved.theme, ThemePreference::Light);
    }

    #[test]
    fn theme_toggle_light_to_dark_to_light_saves_settings_without_reloading_html() {
        let dir = unique_test_dir("theme-toggle-shell-output");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");
        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("theme=\"light\""),
            "default theme should be light in initial output"
        );

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("first toggle should succeed");
        assert_eq!(ui.initial_html().len(), 1, "no additional HTML on toggle");

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("second toggle should succeed");
        assert_eq!(ui.initial_html().len(), 1, "no additional HTML on second toggle");
    }

    #[test]
    fn file_open_preserves_dark_theme_in_rendered_output() {
        use crate::ui::Theme;
        let selected_path = PathBuf::from(r"C:\docs\guide.md");
        let source = SourceDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let rendered = RenderedDocument {
            path: selected_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::selected(selected_path),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
        )
        .with_theme(Theme::Dark);

        controller
            .open_file_requested()
            .expect("open file should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("guide.md"),
        );
        let html = ui
            .last_document_html()
            .expect("document html after file open in dark theme");
        assert!(
            html.contains("theme=\"dark\""),
            "file opened while dark theme is active should render with dark theme"
        );
        assert!(
            html.contains("fill=%22%23D1D5DB%22"),
            "file opened in dark theme should use dark icon assets"
        );
        assert!(!html.contains("theme=\"light\""));
    }

    #[test]
    fn drop_file_preserves_dark_theme_in_rendered_output() {
        use crate::ui::Theme;
        let dir = unique_test_dir("drop-dark-theme-preserve");
        fs::create_dir_all(&dir).expect("create test dir");
        let file_path = dir.join("notes.md");
        fs::write(&file_path, "# Notes").expect("write test file");

        let source = SourceDocument {
            path: file_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: dir.clone(),
            markdown: "# Notes".to_string(),
        };
        let rendered = RenderedDocument {
            path: file_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: dir.clone(),
            html_body: "<h1>Notes</h1>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let (launcher, launched) = StubChildLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(vec![Ok(source)]),
            StubMarkdownRenderer::new(vec![Ok(rendered)]),
            shell,
            ui.clone(),
            launcher,
            (),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")))
        .with_theme(Theme::Dark);

        controller
            .open_dropped_files(vec![file_path])
            .expect("drop should succeed");

        assert_eq!(
            controller
                .state()
                .current_document()
                .map(|d| d.file_name.as_str()),
            Some("notes.md"),
        );
        let html = ui
            .last_document_html()
            .expect("document html after drop in dark theme");
        assert!(
            html.contains("theme=\"dark\""),
            "dropped file while dark theme is active should render with dark theme"
        );
        assert!(
            html.contains("fill=%22%23D1D5DB%22"),
            "dropped file in dark theme should use dark icon assets"
        );
        assert!(!html.contains("theme=\"light\""));
        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn font_settings_requested_updates_body_font_and_renders_on_confirm() {
        let dir = unique_test_dir("font-settings-update-body-font");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>font-applied</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let font_dialog = StubFontDialog::selected("Consolas", 110);
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            font_dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            (),
        )
        .with_body_font(None)
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings request should succeed");

        assert_eq!(
            controller.body_font(),
            &Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            }),
            "body_font should be updated after font dialog confirms"
        );
        assert_eq!(
            ui.document_html(),
            Vec::<String>::new(),
            "document should NOT be re-rendered on font change (incremental update)"
        );
        assert_eq!(
            ui.apply_body_font_calls(),
            vec![Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            })],
            "apply_body_font should be called with selected font"
        );
    }

    #[test]
    fn font_settings_requested_does_nothing_on_cancel() {
        let ui = RecordingViewerUi::default();
        let font_dialog = StubFontDialog::cancelled();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            font_dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(vec![Ok("<html>unused</html>".to_string())]),
            ui.clone(),
            (),
            (),
        )
        .with_body_font(None);

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings cancel should succeed");

        assert_eq!(
            controller.body_font(),
            &None,
            "body_font should remain None after cancel"
        );
        assert!(
            ui.document_html().is_empty(),
            "no re-render should occur on cancel"
        );
    }

    #[test]
    fn font_settings_requested_continues_on_dialog_error() {
        let ui = RecordingViewerUi::default();
        let font_dialog = StubFontDialog::failed("CommDlgExtendedError: 0x0002");
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            font_dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(vec![Ok("<html>unused</html>".to_string())]),
            ui.clone(),
            (),
            (),
        )
        .with_body_font(None);

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings error should not propagate");

        assert_eq!(
            controller.body_font(),
            &None,
            "body_font should remain None after dialog error"
        );
        assert!(
            ui.document_html().is_empty(),
            "no re-render should occur on dialog error"
        );
        assert!(ui.errors().is_empty());
    }

    #[test]
    fn font_settings_confirm_saves_body_font_to_settings_file() {
        let dir = unique_test_dir("font-settings-save");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>font-saved</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let font_dialog = StubFontDialog::selected("Yu Gothic UI", 120);
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            font_dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui,
            (),
            (),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings save should succeed");

        let saved = SettingsFile::with_path(settings_path).load();
        let bfs = saved.body_font.expect("body_font should be saved");
        assert_eq!(bfs.family_name, "Yu Gothic UI");
        assert_eq!(bfs.point_size_tenths, 120);
    }

    #[test]
    fn font_settings_confirm_with_existing_document_renders_with_body_font() {
        let dir = unique_test_dir("font-settings-render-existing-doc");
        fs::create_dir_all(&dir).expect("create test dir");
        let settings_path = dir.join("settings.json");
        SettingsFile::with_path(settings_path.clone())
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: Some(BodyFontSettings {
                    family_name: "Consolas".to_string(),
                    point_size_tenths: 110,
                }),
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            StubFontDialog::selected("Segoe UI", 100),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            (),
        )
        .with_settings_file(SettingsFile::with_path(settings_path));

        controller.start().expect("start should succeed");

        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("Consolas"),
            "initial render should use saved body font"
        );

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings request should succeed");

        assert_eq!(
            controller.body_font(),
            &Some(BodyFontSettings {
                family_name: "Segoe UI".to_string(),
                point_size_tenths: 100,
            }),
            "body_font should be updated to new selection"
        );

        assert!(
            ui.last_document_html().is_none(),
            "document should NOT be re-rendered on font change (incremental update)"
        );
        assert_eq!(
            ui.apply_body_font_calls().last(),
            Some(&Some(BodyFontSettings {
                family_name: "Segoe UI".to_string(),
                point_size_tenths: 100,
            })),
            "apply_body_font should be called with new font"
        );
    }

    #[test]
    fn start_restores_body_font_from_settings_file() {
        let dir = unique_test_dir("font-settings-start-restore");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        SettingsFile::with_path(settings_path)
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: Some(BodyFontSettings {
                    family_name: "Consolas".to_string(),
                    point_size_tenths: 110,
                }),
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");

        assert_eq!(
            controller.body_font(),
            &Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            }),
            "body_font should be restored from settings on startup"
        );

        let initial_html = ui.initial_html().into_iter().next().expect("initial html");
        assert!(
            initial_html.contains("Consolas"),
            "saved body font should be reflected in initial render"
        );
    }

    #[test]
    fn theme_toggle_preserves_body_font_in_settings() {
        let dir = unique_test_dir("font-settings-theme-toggle-preserve");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        SettingsFile::with_path(settings_path.clone())
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: Some(BodyFontSettings {
                    family_name: "Consolas".to_string(),
                    point_size_tenths: 110,
                }),
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui,
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::ThemeToggleRequested)
            .expect("theme toggle should succeed");

        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(saved.theme, ThemePreference::Dark);
        let bfs = saved
            .body_font
            .expect("body_font should be preserved after theme toggle");
        assert_eq!(bfs.family_name, "Consolas");
        assert_eq!(bfs.point_size_tenths, 110);
    }

    #[test]
    fn start_uses_default_font_when_settings_file_has_no_body_font() {
        let dir = unique_test_dir("font-settings-start-no-font");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        SettingsFile::with_path(dir.join("settings.json"))
            .save(&Settings {
                theme: ThemePreference::Dark,
                body_font: None,
                external_editor: None,
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save test settings");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        controller.start().expect("start should succeed");

        assert_eq!(
            controller.body_font(),
            &None,
            "body_font should be None when not saved in settings"
        );
    }

    #[test]
    fn font_settings_confirm_updates_session_display_even_when_save_fails() {
        let invalid_settings_path = PathBuf::from(r"Z:\nonexistent\mdluma\settings.json");
        let shell = RecordingHtmlShell::new(vec![Ok(
            "<html>font-applied-despite-save-failure</html>".to_string(),
        )]);
        let ui = RecordingViewerUi::default();
        let font_dialog = StubFontDialog::selected("Consolas", 110);
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            font_dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            (),
        )
        .with_settings_file(SettingsFile::with_path(invalid_settings_path));

        controller
            .handle_viewer_command(ViewerCommand::FontSettingsRequested)
            .expect("font settings confirm should succeed even when save fails");

        assert_eq!(
            controller.body_font(),
            &Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            }),
            "body_font should be updated in session even when save fails (req 4.3)"
        );
        assert_eq!(
            ui.document_html(),
            Vec::<String>::new(),
            "document should NOT be re-rendered on font change (incremental update)"
        );
        assert_eq!(
            ui.apply_body_font_calls().last(),
            Some(&Some(BodyFontSettings {
                family_name: "Consolas".to_string(),
                point_size_tenths: 110,
            })),
            "apply_body_font should be called even when save fails (req 4.3)"
        );
    }

    #[test]
    fn start_falls_back_to_none_body_font_when_settings_file_has_invalid_json() {
        let dir = unique_test_dir("font-settings-start-invalid-json");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        fs::write(&settings_path, "{invalid json content here").expect("write invalid json");

        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path));

        controller
            .start()
            .expect("start should succeed despite invalid settings");

        assert_eq!(
            controller.body_font(),
            &None,
            "body_font should be None when settings file has invalid JSON (req 4.1)"
        );
    }

    #[derive(Clone)]
    struct StubFontDialog {
        results: Rc<RefCell<Vec<Result<FontDialogResult, ViewerError>>>>,
    }

    impl StubFontDialog {
        fn selected(family: &str, size_tenths: u16) -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(FontDialogResult::Selected(
                    BodyFontSettings {
                        family_name: family.to_string(),
                        point_size_tenths: size_tenths,
                    },
                ))])),
            }
        }

        fn cancelled() -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(FontDialogResult::Cancelled)])),
            }
        }

        fn failed(message: &str) -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Err(ViewerError::font_dialog(message))])),
            }
        }
    }

    impl FontDialog for StubFontDialog {
        fn choose_body_font(
            &self,
            _owner: Option<crate::sciter::ffi::SciterWindowHandle>,
            _initial: Option<&BodyFontSettings>,
        ) -> Result<FontDialogResult, ViewerError> {
            self.results.borrow_mut().remove(0)
        }
    }

    fn rendered_document(file_name: &str) -> RenderedDocument {
        RenderedDocument {
            path: PathBuf::from(r"C:\\docs").join(file_name),
            file_name: file_name.to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: "<p>content</p>".to_string(),
        }
    }

    fn large_markdown_fixture() -> String {
        let mut markdown = String::from("# Large Document\n\n");
        for index in 0..256 {
            markdown.push_str(&format!(
                "## Section {index}\n\n- item {index}\n- item {}\n\nParagraph with [remote link](https://example.com/remote) and ![remote image](https://example.com/image.png).\n\n",
                index + 1
            ));
        }
        markdown
    }

    fn markdown_sanitization_fixture() -> String {
        [
            "# Safe and Unsafe HTML",
            "",
            "<details open><summary>Shortcuts</summary><p><kbd>Ctrl</kbd> + <kbd>K</kbd></p></details>",
            "<div onclick=\"alert(1)\">Inline HTML with event attribute</div>",
            "<iframe src=\"https://example.com/embed\"></iframe>",
            "",
            "[bad script link](javascript:alert(1))",
            "[bad file link](file:///C:/docs/guide.md)",
            "![bad data image](data:text/html;base64,ZXZpbA==)",
        ]
        .join("\n")
    }

    fn unique_test_dir(name: &str) -> TestDir {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("mdluma-{name}-{nonce}"));
        TestDir { path }
    }

    #[derive(Debug)]
    struct TestDir {
        path: PathBuf,
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = PathBuf;

        fn deref(&self) -> &Self::Target {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            if self.path.exists() {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }

    use crate::viewer_launcher::ViewerChildLauncher;

    #[derive(Clone)]
    struct StubChildLauncher {
        launched: Rc<RefCell<Vec<PathBuf>>>,
    }

    impl StubChildLauncher {
        fn new() -> (Self, Rc<RefCell<Vec<PathBuf>>>) {
            let launched = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                },
                launched,
            )
        }
    }

    impl ViewerChildLauncher for StubChildLauncher {
        fn launch_path(
            &self,
            path: &Path,
            _cascade_left: i32,
            _cascade_top: i32,
        ) -> Result<(), ViewerError> {
            self.launched.borrow_mut().push(path.to_path_buf());
            Ok(())
        }
    }

    #[derive(Clone)]
    struct StubFileDialog {
        results: Rc<RefCell<Vec<Result<OpenFileResult, ViewerError>>>>,
        pick_count: Rc<RefCell<usize>>,
        last_owner: Rc<RefCell<Option<crate::sciter::ffi::SciterWindowHandle>>>,
        editor_results: Rc<RefCell<Vec<Result<OpenFileResult, ViewerError>>>>,
    }

    impl StubFileDialog {
        fn selected(path: PathBuf) -> Self {
            Self::sequence(vec![OpenFileResult::Selected(path)])
        }

        fn sequence(results: Vec<OpenFileResult>) -> Self {
            Self {
                results: Rc::new(RefCell::new(results.into_iter().map(Ok).collect())),
                pick_count: Rc::new(RefCell::new(0)),
                last_owner: Rc::new(RefCell::new(None)),
                editor_results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
            }
        }

        fn cancelled() -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
                pick_count: Rc::new(RefCell::new(0)),
                last_owner: Rc::new(RefCell::new(None)),
                editor_results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
            }
        }

        fn pick_count(&self) -> usize {
            *self.pick_count.borrow()
        }

        fn last_owner(&self) -> Option<crate::sciter::ffi::SciterWindowHandle> {
            *self.last_owner.borrow()
        }

        fn with_editor_pick(result: OpenFileResult) -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
                pick_count: Rc::new(RefCell::new(0)),
                last_owner: Rc::new(RefCell::new(None)),
                editor_results: Rc::new(RefCell::new(vec![Ok(result)])),
            }
        }

        fn with_editor_sequence(results: Vec<OpenFileResult>) -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
                pick_count: Rc::new(RefCell::new(0)),
                last_owner: Rc::new(RefCell::new(None)),
                editor_results: Rc::new(RefCell::new(results.into_iter().map(Ok).collect())),
            }
        }

        fn with_editor_error(error: ViewerError) -> Self {
            Self {
                results: Rc::new(RefCell::new(vec![Ok(OpenFileResult::Cancelled)])),
                pick_count: Rc::new(RefCell::new(0)),
                last_owner: Rc::new(RefCell::new(None)),
                editor_results: Rc::new(RefCell::new(vec![Err(error)])),
            }
        }
    }

    impl FileDialog for StubFileDialog {
        fn pick_markdown_file(
            &self,
            owner: Option<crate::sciter::ffi::SciterWindowHandle>,
        ) -> Result<OpenFileResult, ViewerError> {
            *self.pick_count.borrow_mut() += 1;
            *self.last_owner.borrow_mut() = owner;
            self.results.borrow_mut().remove(0)
        }

        fn pick_external_editor_file(
            &self,
            owner: Option<crate::sciter::ffi::SciterWindowHandle>,
        ) -> Result<OpenFileResult, ViewerError> {
            *self.pick_count.borrow_mut() += 1;
            *self.last_owner.borrow_mut() = owner;
            self.editor_results.borrow_mut().remove(0)
        }
    }

    #[derive(Clone)]
    struct StubDocumentLoader {
        results: Rc<RefCell<Vec<Result<SourceDocument, ViewerError>>>>,
        load_count: Rc<RefCell<usize>>,
    }

    impl StubDocumentLoader {
        fn new(results: Vec<Result<SourceDocument, ViewerError>>) -> Self {
            Self {
                results: Rc::new(RefCell::new(results)),
                load_count: Rc::new(RefCell::new(0)),
            }
        }

        fn load_count(&self) -> usize {
            *self.load_count.borrow()
        }
    }

    impl DocumentLoader for StubDocumentLoader {
        fn load(&self, _path: &Path) -> Result<SourceDocument, ViewerError> {
            *self.load_count.borrow_mut() += 1;
            self.results.borrow_mut().remove(0)
        }
    }

    #[derive(Clone)]
    struct StubMarkdownRenderer {
        results: Rc<RefCell<Vec<Result<RenderedDocument, ViewerError>>>>,
        render_count: Rc<RefCell<usize>>,
    }

    impl StubMarkdownRenderer {
        fn new(results: Vec<Result<RenderedDocument, ViewerError>>) -> Self {
            Self {
                results: Rc::new(RefCell::new(results)),
                render_count: Rc::new(RefCell::new(0)),
            }
        }

        fn render_count(&self) -> usize {
            *self.render_count.borrow()
        }
    }

    impl MarkdownRenderer for StubMarkdownRenderer {
        fn render(&self, _source: &SourceDocument) -> Result<RenderedDocument, ViewerError> {
            *self.render_count.borrow_mut() += 1;
            self.results.borrow_mut().remove(0)
        }
    }

    #[derive(Clone)]
    struct RecordingHtmlShell {
        inner: Rc<RefCell<RecordingHtmlShellInner>>,
    }

    struct RecordingHtmlShellInner {
        outputs: Vec<Result<String, ViewerError>>,
        file_names: Vec<Option<String>>,
        html_bodies: Vec<Option<String>>,
        saw_no_document_state: bool,
    }

    impl RecordingHtmlShell {
        fn new(outputs: Vec<Result<String, ViewerError>>) -> Self {
            Self {
                inner: Rc::new(RefCell::new(RecordingHtmlShellInner {
                    outputs,
                    file_names: Vec::new(),
                    html_bodies: Vec::new(),
                    saw_no_document_state: false,
                })),
            }
        }

        fn recorded_state_count(&self) -> usize {
            self.inner.borrow().file_names.len()
        }

        fn saw_no_document_state(&self) -> bool {
            self.inner.borrow().saw_no_document_state
        }

        fn recorded_file_names(&self) -> Vec<Option<String>> {
            self.inner.borrow().file_names.clone()
        }

        fn recorded_html_bodies(&self) -> Vec<Option<String>> {
            self.inner.borrow().html_bodies.clone()
        }
    }

    impl HtmlShell for RecordingHtmlShell {
        fn render_shell(&self, model: ShellModel<'_>) -> Result<String, ViewerError> {
            assert_eq!(model.app_name, APP_NAME);

            let mut inner = self.inner.borrow_mut();
            if model.state.is_no_document() {
                inner.saw_no_document_state = true;
            }

            inner.file_names.push(
                model
                    .state
                    .current_document()
                    .map(|document| document.file_name.clone()),
            );
            inner.html_bodies.push(
                model
                    .state
                    .current_document()
                    .map(|document| document.html_body.clone()),
            );
            inner.outputs.remove(0)
        }
    }

    #[derive(Clone, Default)]
    struct RecordingViewerUi {
        inner: Rc<RefCell<RecordingViewerUiInner>>,
    }

    #[derive(Default)]
    struct RecordingViewerUiInner {
        initial_html: Vec<String>,
        document_html: Vec<String>,
        displayed_document_html: Option<String>,
        errors: Vec<ViewerError>,
        close_requested: bool,
        close_result: Option<Result<(), ViewerError>>,
        native_window_handle: Option<crate::sciter::ffi::SciterWindowHandle>,
        apply_theme_calls: Vec<Theme>,
        apply_body_font_calls: Vec<Option<crate::settings::BodyFontSettings>>,
    }

    impl RecordingViewerUi {
        fn initial_html(&self) -> Vec<String> {
            self.inner.borrow().initial_html.clone()
        }

        fn document_html(&self) -> Vec<String> {
            self.inner.borrow().document_html.clone()
        }

        fn errors(&self) -> Vec<ViewerError> {
            self.inner.borrow().errors.clone()
        }

        fn last_document_html(&self) -> Option<String> {
            self.inner.borrow().document_html.last().cloned()
        }

        fn displayed_document_html(&self) -> Option<String> {
            self.inner.borrow().displayed_document_html.clone()
        }

        fn close_requested(&self) -> bool {
            self.inner.borrow().close_requested
        }

        fn set_close_result(&mut self, result: Option<Result<(), ViewerError>>) {
            self.inner.borrow_mut().close_result = result;
        }

        fn set_native_window_handle(&self, handle: Option<crate::sciter::ffi::SciterWindowHandle>) {
            self.inner.borrow_mut().native_window_handle = handle;
        }


        fn apply_body_font_calls(&self) -> Vec<Option<crate::settings::BodyFontSettings>> {
            self.inner.borrow().apply_body_font_calls.clone()
        }

        fn inject_displayed_document_artifacts(&self, copy_status: &str, selection_marker: &str) {
            let mut inner = self.inner.borrow_mut();
            let Some(document_html) = inner.displayed_document_html.as_mut() else {
                panic!("document html should exist before injecting artifacts");
            };

            *document_html = document_html.replacen(
                "<section class=\"copy-status\" data-copy-status aria-live=\"polite\"></section>",
                &format!(
                    "<section class=\"copy-status\" data-copy-status aria-live=\"polite\">{copy_status}</section>"
                ),
                1,
            );
            *document_html = document_html.replacen(
                "<article class=\"markdown-body\"",
                &format!("<article {selection_marker} class=\"markdown-body\""),
                1,
            );
        }
    }

    impl ViewerUi for RecordingViewerUi {
        fn show_initial(&mut self, html: &str) -> Result<(), ViewerError> {
            self.inner.borrow_mut().initial_html.push(html.to_string());
            Ok(())
        }

        fn show_document(&mut self, html: &str) -> Result<(), ViewerError> {
            let mut inner = self.inner.borrow_mut();
            inner.document_html.push(html.to_string());
            inner.displayed_document_html = Some(html.to_string());
            Ok(())
        }

        fn show_error(&mut self, error: &ViewerError) -> Result<(), ViewerError> {
            self.inner.borrow_mut().errors.push(error.clone());
            Ok(())
        }

        fn run_event_loop(&mut self) -> Result<(), ViewerError> {
            Ok(())
        }

        fn request_close(&mut self) -> Result<(), ViewerError> {
            let mut inner = self.inner.borrow_mut();
            inner.close_requested = true;
            match &inner.close_result {
                Some(result) => result.clone(),
                None => Ok(()),
            }
        }

        fn native_window_handle(&self) -> Option<crate::sciter::ffi::SciterWindowHandle> {
            self.inner.borrow().native_window_handle
        }

        fn apply_theme(&mut self, theme: Theme) -> Result<(), ViewerError> {
            self.inner.borrow_mut().apply_theme_calls.push(theme);
            Ok(())
        }

        fn apply_body_font(
            &mut self,
            body_font: Option<&crate::settings::BodyFontSettings>,
        ) -> Result<(), ViewerError> {
            self.inner
                .borrow_mut()
                .apply_body_font_calls
                .push(body_font.cloned());
            Ok(())
        }
    }

    use crate::external_editor::ExternalEditorLauncher;
    use crate::sciter::window::ViewerCommandHandler;

    #[derive(Clone)]
    struct StubExternalEditorLauncher {
        launched: Rc<RefCell<Vec<(PathBuf, PathBuf)>>>,
        fail: bool,
    }

    impl StubExternalEditorLauncher {
        fn new() -> (Self, Rc<RefCell<Vec<(PathBuf, PathBuf)>>>) {
            let launched = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                    fail: false,
                },
                launched,
            )
        }

        fn failing() -> (Self, Rc<RefCell<Vec<(PathBuf, PathBuf)>>>) {
            let launched = Rc::new(RefCell::new(Vec::new()));
            (
                Self {
                    launched: launched.clone(),
                    fail: true,
                },
                launched,
            )
        }
    }

    impl ExternalEditorLauncher for StubExternalEditorLauncher {
        fn launch(&self, executable: &Path, document_path: &Path) -> Result<(), ViewerError> {
            self.launched
                .borrow_mut()
                .push((executable.to_path_buf(), document_path.to_path_buf()));
            if self.fail {
                Err(ViewerError::external_editor_launch(
                    executable,
                    document_path,
                    "test failure",
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn open_in_external_editor_is_defensive_noop_when_no_document_loaded() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::NoDocument,
        );

        controller
            .open_in_external_editor()
            .expect("no-document should return Ok");

        assert!(
            launched.borrow().is_empty(),
            "launcher must not be called when no document is loaded"
        );
        assert!(
            !ui.close_requested(),
            "close must NOT be called when no document is loaded"
        );
    }

    #[test]
    fn open_in_external_editor_is_defensive_noop_when_only_error_visible() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let error_state = ViewerState::no_document()
            .with_error(ViewerError::file_read("missing.md", "not found"));
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            error_state,
        );

        controller
            .open_in_external_editor()
            .expect("error-visible with no document should return Ok");

        assert!(
            launched.borrow().is_empty(),
            "launcher must not be called when no document backing the error state"
        );
    }

    #[test]
    fn open_in_external_editor_uses_configured_executable_and_current_document_path() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = rendered_document("notes.md");
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_external_editor_config(Some(PathBuf::from(r"C:\Tools\code.exe")));

        controller
            .open_in_external_editor()
            .expect("configured editor launch should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 1, "exactly one launch call per request");
        assert_eq!(
            records[0].0,
            PathBuf::from(r"C:\Tools\code.exe"),
            "must use configured editor executable"
        );
        assert_eq!(records[0].1, doc.path, "must pass current document path");
        assert_ne!(
            records[0].0,
            PathBuf::from("notepad.exe"),
            "must NOT fall back to notepad.exe when editor is configured"
        );
    }



    #[test]
    fn open_in_external_editor_falls_back_to_notepad_when_no_configured_editor() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = rendered_document("guide.md");
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_external_editor_config(None);

        controller
            .open_in_external_editor()
            .expect("notepad fallback launch should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].0,
            PathBuf::from("notepad.exe"),
            "must use notepad.exe when no editor is configured"
        );
        assert_eq!(records[0].1, doc.path);
    }

    #[test]
    fn open_in_external_editor_uses_new_document_path_after_switch() {
        let first_path = PathBuf::from(r"C:\docs\guide.md");
        let second_path = PathBuf::from(r"C:\docs\notes.md");
        let first_source = SourceDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Guide".to_string(),
        };
        let second_source = SourceDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            markdown: "# Notes".to_string(),
        };
        let first_rendered = RenderedDocument {
            path: first_path.clone(),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        };
        let second_rendered = RenderedDocument {
            path: second_path.clone(),
            file_name: "notes.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Notes</h1>".to_string(),
        };
        let shell = RecordingHtmlShell::new(vec![
            Ok("<html>guide</html>".to_string()),
            Ok("<html>notes</html>".to_string()),
        ]);
        let dialog = StubFileDialog::sequence(vec![
            OpenFileResult::Selected(first_path.clone()),
            OpenFileResult::Selected(second_path.clone()),
        ]);
        let loader = StubDocumentLoader::new(vec![Ok(first_source), Ok(second_source)]);
        let renderer = StubMarkdownRenderer::new(vec![Ok(first_rendered), Ok(second_rendered)]);
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            dialog,
            (),
            loader,
            renderer,
            shell,
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::NoDocument,
        );

        controller
            .open_file_requested()
            .expect("first open should succeed");
        controller
            .open_in_external_editor()
            .expect("first external editor request should succeed");

        controller
            .open_file_requested()
            .expect("second open should succeed");
        controller
            .open_in_external_editor()
            .expect("second external editor request should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 2, "one launch per external editor request");
        assert_eq!(
            records[0].1, first_path,
            "first request must use first document path"
        );
        assert_eq!(
            records[1].1, second_path,
            "second request must use new (switched) document path"
        );
    }



    #[test]
    fn open_in_external_editor_command_dispatches_to_launcher_with_configured_editor() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = rendered_document("cmd.md");
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc),
        )
        .with_external_editor_config(Some(PathBuf::from(r"C:\Tools\editor.exe")));

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect("external editor command should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0].0,
            PathBuf::from(r"C:\Tools\editor.exe"),
            "command dispatch must use configured editor"
        );
    }

    #[test]
    fn open_in_external_editor_launch_success_requests_close() {
        let (editor_launcher, _launched) = StubExternalEditorLauncher::new();
        let ui = RecordingViewerUi::default();
        let doc = rendered_document("close.md");
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc),
        );

        controller
            .open_in_external_editor()
            .expect("launch success should return Ok");

        assert!(
            ui.close_requested(),
            "request_close must be called after successful launch (req 4.1)"
        );
    }

    #[test]
    fn open_in_external_editor_close_failure_shows_error_and_preserves_document() {
        let (editor_launcher, _launched) = StubExternalEditorLauncher::new();
        let mut ui = RecordingViewerUi::default();
        ui.set_close_result(Some(Err(ViewerError::ui("close failed"))));
        let doc = rendered_document("close-fail.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>close-error</html>".to_string())]);
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        );

        let result = controller.open_in_external_editor();
        assert!(result.is_err(), "close failure must propagate as error");
        assert!(
            ui.close_requested(),
            "request_close must still be attempted even when it fails"
        );
        assert!(
            controller.state().is_error_visible(),
            "state must transition to ErrorVisible on close failure"
        );
        assert_eq!(
            controller.state().current_document(),
            Some(&doc),
            "current document must be preserved on close failure (req 5.3)"
        );
    }

    #[test]
    fn open_in_external_editor_launch_failure_shows_error_and_preserves_document() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::failing();
        let ui = RecordingViewerUi::default();
        let doc = rendered_document("launch-fail.md");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>launch-error</html>".to_string())]);
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        );

        let result = controller.open_in_external_editor();
        assert!(
            result.is_err(),
            "launch failure must propagate as error (req 5.1)"
        );
        assert!(
            !ui.close_requested(),
            "close must NOT be called on launch failure (req 5.2)"
        );
        assert!(
            controller.state().is_error_visible(),
            "state must transition to ErrorVisible on launch failure"
        );
        assert_eq!(
            controller.state().current_document(),
            Some(&doc),
            "current document must be preserved on launch failure (req 5.3)"
        );
        assert_eq!(
            launched.borrow().len(),
            1,
            "launcher must still be called once"
        );
    }

    #[test]
    fn open_in_external_editor_command_is_noop_when_no_document() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::NoDocument,
        );

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect("command should be defensive noop without document");

        assert!(launched.borrow().is_empty());
    }

    #[test]
    fn e2e_external_editor_launch_failure_renders_error_html_with_preserved_file_name_and_enabled_control(
    ) {
        let (editor_launcher, launched) = StubExternalEditorLauncher::failing();
        let doc = RenderedDocument {
            path: PathBuf::from(r"C:\docs\sample.md"),
            file_name: "sample.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Sample</h1><p>Content preserved after failure.</p>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_external_editor_config(Some(PathBuf::from(r"C:\Tools\bad_editor.exe")));

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect_err("launch failure must propagate (req 5.1)");

        assert_eq!(launched.borrow().len(), 1, "launcher must be called");
        assert!(
            !ui.close_requested(),
            "close must NOT be called on launch failure (req 5.2)"
        );
        assert!(
            controller.state().is_error_visible(),
            "state must be ErrorVisible after launch failure"
        );
        assert_eq!(
            controller.state().current_document(),
            Some(&doc),
            "current document must be preserved (req 5.3)"
        );

        let rendered_html = ui
            .last_document_html()
            .expect("error state must render HTML to UI (req 5.1)");

        assert!(
            rendered_html.contains("external editor"),
            "rendered HTML must mention 'external editor' in error message (req 5.1)"
        );
        assert!(
            rendered_html.contains("sample.md"),
            "rendered HTML must preserve the file name (req 5.3)"
        );
        assert!(
            rendered_html.contains("<h1>Sample</h1>"),
            "rendered HTML must preserve document content body (req 5.3)"
        );
        assert!(
            rendered_html.contains("data-error-overlay"),
            "rendered HTML must contain the error overlay marker (req 5.1)"
        );
        assert!(
            !rendered_html.contains("data-action=\"external-editor\" disabled"),
            "external editor control must remain enabled when document is loaded (req 1.3)"
        );
        assert!(
            rendered_html.contains("data-action=\"external-editor\""),
            "external editor action must be present in rendered HTML"
        );
    }

    #[test]
    fn e2e_external_editor_success_requests_close_and_does_not_show_error() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = RenderedDocument {
            path: PathBuf::from(r"C:\docs\complete.md"),
            file_name: "complete.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Complete</h1>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc),
        )
        .with_external_editor_config(Some(PathBuf::from(r"C:\Tools\editor.exe")));

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect("successful launch + close must return Ok (req 4.1)");

        let records = launched.borrow();
        assert_eq!(records.len(), 1, "exactly one launch call");
        assert_eq!(
            records[0].0,
            PathBuf::from(r"C:\Tools\editor.exe"),
            "must use configured executable (req 2.1)"
        );
        assert_eq!(
            records[0].1,
            PathBuf::from(r"C:\docs\complete.md"),
            "must pass current document path (req 3.1)"
        );
        drop(records);

        assert!(
            ui.close_requested(),
            "request_close must be called after successful launch (req 4.1)"
        );
        assert!(
            !controller.state().is_error_visible(),
            "state must NOT be ErrorVisible on success"
        );
        assert!(
            ui.document_html().is_empty(),
            "no re-render to UI on successful launch+close path"
        );
        assert!(
            ui.errors().is_empty(),
            "no error shown to UI on successful launch+close path"
        );
    }

    #[test]
    fn e2e_no_document_command_renders_disabled_editor_and_does_not_launch_or_close() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::NoDocument,
        );

        controller
            .start()
            .expect("start should render initial shell");

        let initial_html = ui
            .initial_html()
            .into_iter()
            .next()
            .expect("initial HTML rendered");

        assert!(
            initial_html.contains("data-action=\"external-editor\" disabled"),
            "initial shell must have external editor disabled (req 1.2)"
        );
        assert!(
            initial_html.contains("Open Markdown file"),
            "initial shell must show empty state prompt"
        );

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect("no-document command must be defensive noop");

        assert!(
            launched.borrow().is_empty(),
            "launcher must not be called without a document"
        );
        assert!(
            !ui.close_requested(),
            "close must not be requested without a document"
        );
    }

    #[test]
    fn external_editor_setting_selected_updates_memory_and_persists_to_settings() {
        let dir = unique_test_dir("ext-editor-selected-save");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let selected_path = PathBuf::from(r"C:\Tools\notepadpp.exe");
        let mut controller = AppController::new(
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(selected_path.clone())),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        let result = controller.open_external_editor_setting();
        assert!(
            result.is_ok(),
            "open_external_editor_setting should succeed on Selected"
        );

        assert_eq!(
            controller.external_editor(),
            &Some(selected_path.clone()),
            "external_editor must be set in memory after Selected"
        );

        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(
            saved.external_editor,
            Some(selected_path),
            "external_editor must be persisted to settings file after Selected"
        );
    }

    #[test]
    fn external_editor_setting_cancelled_preserves_none_config() {
        let dir = unique_test_dir("ext-editor-cancel-none");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let mut controller = AppController::new(
            StubFileDialog::with_editor_pick(OpenFileResult::Cancelled),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        let result = controller.open_external_editor_setting();
        assert!(
            result.is_ok(),
            "open_external_editor_setting should succeed on Cancelled"
        );
        assert_eq!(
            controller.external_editor(),
            &None,
            "external_editor must remain None after cancelled dialog"
        );
    }



    #[test]
    fn external_editor_setting_cancelled_preserves_existing_config() {
        let dir = unique_test_dir("ext-editor-cancel-preserve");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let existing_path = PathBuf::from(r"C:\Tools\code.exe");
        SettingsFile::with_path(settings_path.clone())
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: None,
                external_editor: Some(existing_path.clone()),
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save test settings");

        let dialog = StubFileDialog::with_editor_pick(OpenFileResult::Cancelled);
        let mut controller = AppController::new(
            dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path));

        controller
            .open_external_editor_setting()
            .expect("cancelled setting should succeed");

        assert_eq!(
            controller.external_editor(),
            &Some(existing_path),
            "external_editor must remain unchanged after cancelled dialog"
        );
    }

    #[test]
    fn external_editor_setting_second_select_overwrites_first() {
        let dir = unique_test_dir("ext-editor-select-overwrite");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let first_path = PathBuf::from(r"C:\Tools\code.exe");
        let second_path = PathBuf::from(r"C:\Tools\vim.exe");
        let dialog = StubFileDialog::with_editor_sequence(vec![
            OpenFileResult::Selected(first_path.clone()),
            OpenFileResult::Selected(second_path.clone()),
        ]);
        let mut controller = AppController::new(
            dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .open_external_editor_setting()
            .expect("first select should succeed");
        assert_eq!(
            controller.external_editor(),
            &Some(first_path),
            "first select should set external_editor"
        );

        controller
            .open_external_editor_setting()
            .expect("second select should succeed");
        assert_eq!(
            controller.external_editor(),
            &Some(second_path.clone()),
            "second select should overwrite external_editor"
        );

        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(
            saved.external_editor,
            Some(second_path),
            "second selected path must be persisted, replacing the first"
        );
    }

    #[test]
    fn external_editor_setting_save_failure_shows_error_and_preserves_session() {
        let invalid_settings_path = PathBuf::from(r"Z:\nonexistent\mdluma\settings.json");
        let selected_path = PathBuf::from(r"C:\Tools\editor.exe");
        let shell = RecordingHtmlShell::new(vec![Ok("<html>save-error</html>".to_string())]);
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(selected_path.clone())),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(invalid_settings_path));

        let result = controller.open_external_editor_setting();
        assert!(
            result.is_ok(),
            "open_external_editor_setting should succeed even when save fails"
        );

        assert!(
            controller.state().is_error_visible(),
            "state must be ErrorVisible after save failure (req 4.3)"
        );
        assert_eq!(
            ui.document_html(),
            vec!["<html>save-error</html>".to_string()],
            "error HTML must be rendered to UI on save failure (req 4.3)"
        );
        assert!(
            !ui.close_requested(),
            "close must NOT be called on save failure (session continues, req 4.3)"
        );
        assert_eq!(
            controller.external_editor(),
            &Some(selected_path),
            "external_editor must be set in memory even when save fails"
        );
    }

    #[test]
    fn save_failure_acceptance_no_document_shows_user_visible_error_and_continues_session() {
        let invalid_settings_path = PathBuf::from(r"Z:\nonexistent\mdluma\settings.json");
        let selected_path = PathBuf::from(r"C:\Tools\editor.exe");
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::new(
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(selected_path)),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
        )
        .with_settings_file(SettingsFile::with_path(invalid_settings_path));

        let result = controller.open_external_editor_setting();
        assert!(
            result.is_ok(),
            "session must continue after save failure (req 4.3)"
        );

        assert!(
            controller.state().is_error_visible(),
            "state must be ErrorVisible after save failure (req 4.3)"
        );
        assert!(
            !ui.close_requested(),
            "close must NOT be called on save failure (session continues, req 4.3)"
        );

        let rendered_html = ui
            .last_document_html()
            .expect("error HTML must be rendered to UI on save failure (req 4.3)");
        assert!(
            rendered_html.contains("MDLuma could not save application settings."),
            "rendered HTML must contain the user-facing settings save error message (req 4.3)"
        );
        assert!(
            rendered_html.contains("data-error-area"),
            "rendered HTML must contain the error area marker (req 4.3)"
        );
        assert!(
            rendered_html.contains("<p class=\"error-message\">"),
            "rendered HTML must contain error message paragraph (req 4.3)"
        );
    }

    #[test]
    fn save_failure_acceptance_document_loaded_shows_error_and_preserves_document() {
        let invalid_settings_path = PathBuf::from(r"Z:\nonexistent\mdluma\settings.json");
        let selected_path = PathBuf::from(r"C:\Tools\editor.exe");
        let doc = RenderedDocument {
            path: PathBuf::from(r"C:\docs\keep.md"),
            file_name: "keep.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Keep</h1><p>Document preserved after save failure.</p>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_state(
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(selected_path)),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            ViewerState::document_loaded(doc.clone()),
        )
        .with_settings_file(SettingsFile::with_path(invalid_settings_path));

        let result = controller.open_external_editor_setting();
        assert!(
            result.is_ok(),
            "session must continue after save failure even with document loaded (req 4.3)"
        );

        assert!(
            controller.state().is_error_visible(),
            "state must be ErrorVisible after save failure with document loaded (req 4.3)"
        );
        assert_eq!(
            controller.state().current_document(),
            Some(&doc),
            "current document must be accessible after save failure (req 4.3)"
        );
        assert!(
            controller.state().current_document().is_some(),
            "document count must remain 1 after save failure (req 4.3)"
        );
        assert!(
            !ui.close_requested(),
            "close must NOT be called on save failure (browsing continues, req 4.3)"
        );

        let rendered_html = ui
            .last_document_html()
            .expect("error HTML must be rendered to UI on save failure (req 4.3)");
        assert!(
            rendered_html.contains("MDLuma could not save application settings."),
            "rendered HTML must contain the user-facing settings save error message (req 4.3)"
        );
        assert!(
            rendered_html.contains("data-error-overlay"),
            "rendered HTML must contain the error overlay marker (req 4.3)"
        );
        assert!(
            rendered_html.contains("keep.md"),
            "rendered HTML must preserve the current file name after save failure (req 4.3)"
        );
        assert!(
            rendered_html.contains("<h1>Keep</h1>"),
            "rendered HTML must preserve document body content after save failure (req 4.3)"
        );
        assert!(
            rendered_html.contains("Document preserved after save failure."),
            "rendered HTML must preserve document paragraph content after save failure (req 4.3)"
        );
    }

    #[test]
    fn launch_failure_acceptance_renders_user_visible_error_message_in_html() {
        let (editor_launcher, launched) = StubExternalEditorLauncher::failing();
        let doc = RenderedDocument {
            path: PathBuf::from(r"C:\docs\launch-fail.md"),
            file_name: "launch-fail.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<h1>Launch Fail</h1><p>Document retained.</p>".to_string(),
        };
        let shell =
            crate::html_shell::DefaultHtmlShell::new(crate::ui::EmbeddedUiAssets::default());
        let ui = RecordingViewerUi::default();
        let mut controller = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            shell,
            ui.clone(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_external_editor_config(Some(PathBuf::from(r"C:\Tools\bad_editor.exe")));

        ViewerCommandHandler::handle_viewer_command(
            &mut controller,
            ViewerCommand::ExternalEditorRequested,
        )
        .expect_err("launch failure must propagate as error (req 4.1)");

        assert_eq!(launched.borrow().len(), 1, "launcher must be called once");
        assert!(
            !ui.close_requested(),
            "close must NOT be called on launch failure (req 4.2)"
        );
        assert!(
            controller.state().is_error_visible(),
            "state must be ErrorVisible after launch failure (req 4.1)"
        );
        assert_eq!(
            controller.state().current_document(),
            Some(&doc),
            "current document must be accessible after launch failure (req 4.2)"
        );

        let rendered_html = ui
            .last_document_html()
            .expect("error state must render HTML to UI (req 4.1)");
        assert!(
            rendered_html.contains("MDLuma could not open the file in the external editor."),
            "rendered HTML must contain the user-facing external editor launch error message (req 4.1)"
        );
        assert!(
            rendered_html.contains("data-error-overlay"),
            "rendered HTML must contain the error overlay marker (req 4.1)"
        );
        assert!(
            rendered_html.contains(r#"data-action="error-ok">OK<"#),
            "rendered HTML must contain the error dialog dismiss button (req 4.1)"
        );
        assert!(
            rendered_html.contains("launch-fail.md"),
            "rendered HTML must preserve the file name after launch failure (req 4.2)"
        );
        assert!(
            rendered_html.contains("<h1>Launch Fail</h1>"),
            "rendered HTML must preserve document body content after launch failure (req 4.2)"
        );
    }



    #[test]
    fn with_settings_file_reloads_external_editor() {
        let dir = unique_test_dir("ext-editor-settings-reload");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let saved_path = PathBuf::from(r"C:\Tools\editor.exe");

        SettingsFile::with_path(settings_path.clone())
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: None,
                external_editor: Some(saved_path.clone()),
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save settings");

        let (editor_launcher, _launched) = StubExternalEditorLauncher::new();
        let mut controller = AppController::with_launchers(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
        )
        .with_external_editor_config(None);

        assert_eq!(
            controller.external_editor(),
            &None,
            "should start with no editor configured"
        );

        controller = controller.with_settings_file(SettingsFile::with_path(settings_path));

        assert_eq!(
            controller.external_editor(),
            &Some(saved_path),
            "with_settings_file must reload external_editor from settings file"
        );
    }

    #[test]
    fn handle_viewer_command_external_editor_setting_selected_updates_memory_and_persists() {
        let dir = unique_test_dir("ext-editor-cmd-selected-flow");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let selected_path = PathBuf::from(r"C:\Tools\notepadpp.exe");
        let dialog =
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(selected_path.clone()));
        let mut controller = AppController::new(
            dialog.clone(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::ExternalEditorSettingRequested)
            .expect("command dispatch should succeed");

        assert_eq!(
            dialog.pick_count(),
            1,
            "handle_viewer_command must call pick_external_editor_file"
        );
        assert_eq!(
            controller.external_editor(),
            &Some(selected_path.clone()),
            "external_editor must be updated via command dispatch"
        );
        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(
            saved.external_editor,
            Some(selected_path),
            "settings file must be saved via command dispatch flow"
        );
    }

    #[test]
    fn handle_viewer_command_external_editor_setting_cancelled_preserves_state() {
        let dir = unique_test_dir("ext-editor-cmd-cancelled");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let existing_path = PathBuf::from(r"C:\Tools\vim.exe");
        SettingsFile::with_path(settings_path.clone())
            .save(&Settings {
                theme: ThemePreference::Light,
                body_font: None,
                external_editor: Some(existing_path.clone()),
                recent_files: vec![],
                window_geometry: None,
                content_max_width_px: DEFAULT_CONTENT_MAX_WIDTH_PX,
            })
            .expect("save initial settings");
        let dialog = StubFileDialog::with_editor_pick(OpenFileResult::Cancelled);
        let mut controller = AppController::new(
            dialog.clone(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller
            .handle_viewer_command(ViewerCommand::ExternalEditorSettingRequested)
            .expect("command dispatch should succeed on cancelled");

        assert_eq!(
            dialog.pick_count(),
            1,
            "handle_viewer_command must call pick_external_editor_file even on cancelled"
        );
        assert_eq!(
            controller.external_editor(),
            &Some(existing_path.clone()),
            "existing external_editor must be preserved after cancelled via command dispatch"
        );
        let saved = SettingsFile::with_path(settings_path).load();
        assert_eq!(
            saved.external_editor,
            Some(existing_path),
            "settings file must be unchanged after cancelled via command dispatch"
        );
    }

    #[test]
    fn external_editor_setting_dialog_error_propagates_and_session_does_not_crash() {
        let dir = unique_test_dir("ext-editor-dialog-error");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let dialog = StubFileDialog::with_editor_error(ViewerError::file_dialog("dialog failure"));
        let mut controller = AppController::new(
            dialog.clone(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(dir.join("settings.json")));

        let result =
            controller.handle_viewer_command(ViewerCommand::ExternalEditorSettingRequested);
        assert!(
            result.is_err(),
            "dialog error must propagate through command handler"
        );
        assert!(
            !controller.state().is_error_visible(),
            "session state must not crash on dialog error"
        );
        assert_eq!(
            controller.external_editor(),
            &None,
            "external_editor must remain unchanged after dialog error"
        );
        assert_eq!(
            dialog.pick_count(),
            1,
            "dialog pick_external_editor_file must have been called"
        );
    }

    #[test]
    fn e2e_save_restart_launch_full_chain() {
        let dir = unique_test_dir("ext-editor-e2e-full-chain");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let editor_path = PathBuf::from(r"C:\Tools\myeditor.exe");

        let dialog =
            StubFileDialog::with_editor_pick(OpenFileResult::Selected(editor_path.clone()));
        let mut controller_a = AppController::new(
            dialog,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller_a
            .open_external_editor_setting()
            .expect("dialog + save should succeed");
        assert_eq!(
            controller_a.external_editor(),
            &Some(editor_path.clone()),
            "in-memory config must reflect saved editor"
        );

        let saved = SettingsFile::with_path(settings_path.clone()).load();
        assert_eq!(
            saved.external_editor,
            Some(editor_path.clone()),
            "settings file on disk must contain saved editor path"
        );

        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = rendered_document("readme.md");
        let mut controller_b = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_settings_file(SettingsFile::with_path(settings_path));

        assert_eq!(
            controller_b.external_editor(),
            &Some(editor_path.clone()),
            "restarted controller must load saved external_editor"
        );

        controller_b
            .open_in_external_editor()
            .expect("launch should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 1, "exactly one launch call");
        assert_eq!(
            records[0].0, editor_path,
            "must use editor executable loaded from settings"
        );
        assert_eq!(records[0].1, doc.path, "must pass current document path");
        assert_ne!(
            records[0].0,
            PathBuf::from("notepad.exe"),
            "must NOT fall back to notepad.exe when editor is configured via settings"
        );
    }

    #[test]
    fn e2e_multiple_round_trips_save_restart_save_restart() {
        let dir = unique_test_dir("ext-editor-roundtrips");
        fs::create_dir_all(dir.as_ref()).expect("create test dir");
        let settings_path = dir.join("settings.json");
        let editor_a = PathBuf::from(r"C:\Tools\editor_a.exe");
        let editor_b = PathBuf::from(r"C:\Tools\editor_b.exe");

        let dialog_a = StubFileDialog::with_editor_pick(OpenFileResult::Selected(editor_a.clone()));
        let mut controller_1 = AppController::new(
            dialog_a,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller_1
            .open_external_editor_setting()
            .expect("save editor A should succeed");
        assert_eq!(controller_1.external_editor(), &Some(editor_a.clone()));

        let controller_2 = AppController::new(
            StubFileDialog::cancelled(),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        assert_eq!(
            controller_2.external_editor(),
            &Some(editor_a.clone()),
            "after first restart, editor A must be loaded"
        );

        let dialog_b = StubFileDialog::with_editor_pick(OpenFileResult::Selected(editor_b.clone()));
        let mut controller_3 = AppController::new(
            dialog_b,
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
        )
        .with_settings_file(SettingsFile::with_path(settings_path.clone()));

        controller_3
            .open_external_editor_setting()
            .expect("save editor B should succeed");
        assert_eq!(controller_3.external_editor(), &Some(editor_b.clone()));

        let saved = SettingsFile::with_path(settings_path.clone()).load();
        assert_eq!(
            saved.external_editor,
            Some(editor_b.clone()),
            "settings file must contain editor B after second save"
        );

        let (editor_launcher, launched) = StubExternalEditorLauncher::new();
        let doc = rendered_document("final.md");
        let mut controller_4 = AppController::with_external_editor_launcher_and_state(
            StubFileDialog::cancelled(),
            (),
            StubDocumentLoader::new(Vec::new()),
            StubMarkdownRenderer::new(Vec::new()),
            RecordingHtmlShell::new(Vec::new()),
            RecordingViewerUi::default(),
            (),
            editor_launcher,
            ViewerState::document_loaded(doc.clone()),
        )
        .with_settings_file(SettingsFile::with_path(settings_path));

        assert_eq!(
            controller_4.external_editor(),
            &Some(editor_b.clone()),
            "after second restart, editor B must be loaded"
        );

        controller_4
            .open_in_external_editor()
            .expect("launch with editor B should succeed");

        let records = launched.borrow();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, editor_b, "must use editor B from settings");
        assert_eq!(records[0].1, doc.path);
    }
}

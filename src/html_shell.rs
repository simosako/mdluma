use crate::html_sanitizer::{file_url_from_path, sanitize_body_html};
use crate::settings::BodyFontSettings;
use crate::ui::{EmbeddedUiAssets, IconName, IconTheme, Theme, UiTextAsset};
use crate::{ViewerError, ViewerState};
use std::path::PathBuf;

pub struct ShellModel<'a> {
    pub app_name: &'a str,
    pub state: &'a ViewerState,
    pub theme: Theme,
    pub body_font: Option<&'a BodyFontSettings>,
    pub recent_files: &'a [PathBuf],
}

pub trait HtmlShell {
    fn render_shell(&self, model: ShellModel<'_>) -> Result<String, ViewerError>;
}

#[derive(Debug, Clone)]
pub struct DefaultHtmlShell {
    assets: EmbeddedUiAssets,
}

impl DefaultHtmlShell {
    pub fn new(assets: EmbeddedUiAssets) -> Self {
        Self { assets }
    }
}

impl HtmlShell for DefaultHtmlShell {
    fn render_shell(&self, model: ShellModel<'_>) -> Result<String, ViewerError> {
        let template = self.assets.read_text_asset(UiTextAsset::IndexHtml)?;
        let styles = self.assets.read_text_asset(UiTextAsset::StylesCss)?;
        let script = self.assets.read_text_asset(UiTextAsset::AppJs)?;
        let icon_theme = model.theme.icon_theme();
        let app_icon = icon_urls(&self.assets, IconName::App, icon_theme)?;
        let open_icon = icon_urls(&self.assets, IconName::Open, icon_theme)?;
        let search_icon = icon_urls(&self.assets, IconName::Search, icon_theme)?;
        let search_prev_icon = icon_urls(&self.assets, IconName::SearchPrev, icon_theme)?;
        let search_next_icon = icon_urls(&self.assets, IconName::SearchNext, icon_theme)?;
        let search_close_icon = icon_urls(&self.assets, IconName::SearchClose, icon_theme)?;
        let more_icon = icon_urls(&self.assets, IconName::More, icon_theme)?;
        let window_minimize_icon = icon_urls(&self.assets, IconName::WindowMinimize, icon_theme)?;
        let window_maximize_icon = icon_urls(&self.assets, IconName::WindowMaximize, icon_theme)?;
        let window_restore_icon = icon_urls(&self.assets, IconName::WindowRestore, icon_theme)?;
        let window_close_icon = icon_urls(&self.assets, IconName::WindowClose, icon_theme)?;
        let toggle_icon_url = self
            .assets
            .icon_data_url(model.theme.toggle_icon(), icon_theme)?;
        let toggle_icon_url_light = self
            .assets
            .icon_data_url(IconName::Moon, IconTheme::Light)?;
        let toggle_icon_url_dark = self.assets.icon_data_url(IconName::Sun, IconTheme::Dark)?;
        let file_name = current_file_name(model.state, model.recent_files);
        let base_href = current_document_base_href(model.state);
        let content = content_html(model.state);
        let error = error_html(model.state);
        let error_overlay = error_overlay_html(model.state);
        let body_font_style = body_font_css(model.body_font);
        let external_editor_disabled = external_editor_disabled_attr(model.state);
        let recent_files_html = recent_files_html(model.recent_files);
        let app_name = escape_html(model.app_name);
        let current_file_name = escape_html(file_name);
        let replacements = [
            ("{{APP_NAME}}", app_name.as_str()),
            ("{{CURRENT_FILE_NAME}}", current_file_name.as_str()),
            ("{{BASE_HREF}}", base_href.as_str()),
            ("{{CONTENT}}", content.as_str()),
            ("{{ERROR}}", error.as_str()),
            ("{{STYLES}}", styles.as_ref()),
            ("{{BODY_FONT_STYLE}}", body_font_style.as_str()),
            ("{{SCRIPT}}", script.as_ref()),
            ("{{APP_ICON}}", app_icon.current.as_str()),
            ("{{APP_ICON_LIGHT}}", app_icon.light.as_str()),
            ("{{APP_ICON_DARK}}", app_icon.dark.as_str()),
            ("{{OPEN_ICON}}", open_icon.current.as_str()),
            ("{{OPEN_ICON_LIGHT}}", open_icon.light.as_str()),
            ("{{OPEN_ICON_DARK}}", open_icon.dark.as_str()),
            ("{{SEARCH_ICON}}", search_icon.current.as_str()),
            ("{{SEARCH_ICON_LIGHT}}", search_icon.light.as_str()),
            ("{{SEARCH_ICON_DARK}}", search_icon.dark.as_str()),
            ("{{SEARCH_PREV_ICON}}", search_prev_icon.current.as_str()),
            ("{{SEARCH_PREV_ICON_LIGHT}}", search_prev_icon.light.as_str()),
            ("{{SEARCH_PREV_ICON_DARK}}", search_prev_icon.dark.as_str()),
            ("{{SEARCH_NEXT_ICON}}", search_next_icon.current.as_str()),
            ("{{SEARCH_NEXT_ICON_LIGHT}}", search_next_icon.light.as_str()),
            ("{{SEARCH_NEXT_ICON_DARK}}", search_next_icon.dark.as_str()),
            ("{{SEARCH_CLOSE_ICON}}", search_close_icon.current.as_str()),
            ("{{SEARCH_CLOSE_ICON_LIGHT}}", search_close_icon.light.as_str()),
            ("{{SEARCH_CLOSE_ICON_DARK}}", search_close_icon.dark.as_str()),
            ("{{MORE_ICON}}", more_icon.current.as_str()),
            ("{{MORE_ICON_LIGHT}}", more_icon.light.as_str()),
            ("{{MORE_ICON_DARK}}", more_icon.dark.as_str()),
            ("{{WINDOW_MINIMIZE_ICON}}", window_minimize_icon.current.as_str()),
            ("{{WINDOW_MINIMIZE_ICON_LIGHT}}", window_minimize_icon.light.as_str()),
            ("{{WINDOW_MINIMIZE_ICON_DARK}}", window_minimize_icon.dark.as_str()),
            ("{{WINDOW_MAXIMIZE_ICON}}", window_maximize_icon.current.as_str()),
            ("{{WINDOW_MAXIMIZE_ICON_LIGHT}}", window_maximize_icon.light.as_str()),
            ("{{WINDOW_MAXIMIZE_ICON_DARK}}", window_maximize_icon.dark.as_str()),
            ("{{WINDOW_RESTORE_ICON_LIGHT}}", window_restore_icon.light.as_str()),
            ("{{WINDOW_RESTORE_ICON_DARK}}", window_restore_icon.dark.as_str()),
            ("{{WINDOW_CLOSE_ICON}}", window_close_icon.current.as_str()),
            ("{{WINDOW_CLOSE_ICON_LIGHT}}", window_close_icon.light.as_str()),
            ("{{WINDOW_CLOSE_ICON_DARK}}", window_close_icon.dark.as_str()),
            ("{{THEME_ATTR}}", model.theme.theme_attr()),
            ("{{THEME_ICON}}", toggle_icon_url.as_str()),
            ("{{THEME_ICON_LIGHT}}", toggle_icon_url_light.as_str()),
            ("{{THEME_ICON_DARK}}", toggle_icon_url_dark.as_str()),
            ("{{VERSION}}", env!("CARGO_PKG_VERSION")),
            ("{{BUILD_NUMBER}}", env!("GIT_COMMIT_HASH")),
            (
                "{{SCITER_ATTRIBUTION}}",
                "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc.",
            ),
            ("{{EXTERNAL_EDITOR_DISABLED}}", external_editor_disabled),
            ("{{ERROR_OVERLAY}}", error_overlay.as_str()),
            ("{{RECENT_FILES}}", recent_files_html.as_str()),
        ];

        let html = render_template(&template, &replacements);

        Ok(html)
    }
}

struct IconUrls {
    current: String,
    light: String,
    dark: String,
}

fn icon_urls(
    assets: &EmbeddedUiAssets,
    name: IconName,
    current_theme: IconTheme,
) -> Result<IconUrls, ViewerError> {
    Ok(IconUrls {
        current: assets.icon_data_url(name, current_theme)?,
        light: assets.icon_data_url(name, IconTheme::Light)?,
        dark: assets.icon_data_url(name, IconTheme::Dark)?,
    })
}

fn render_template(template: &str, replacements: &[(&str, &str)]) -> String {
    let mut rendered = String::with_capacity(template.len());
    let mut cursor = 0;

    while let Some(relative_start) = template[cursor..].find("{{") {
        let token_start = cursor + relative_start;
        rendered.push_str(&template[cursor..token_start]);

        let Some(relative_end) = template[token_start + 2..].find("}}") else {
            rendered.push_str(&template[token_start..]);
            return rendered;
        };
        let token_end = token_start + 2 + relative_end + 2;
        let token = &template[token_start..token_end];

        match replacements.iter().find(|(key, _)| *key == token) {
            Some((_, value)) => rendered.push_str(value),
            None => rendered.push_str(token),
        }

        cursor = token_end;
    }

    rendered.push_str(&template[cursor..]);
    rendered
}

fn external_editor_disabled_attr(state: &ViewerState) -> &'static str {
    if state.current_document().is_some() {
        ""
    } else {
        "disabled"
    }
}

fn recent_files_html(recent_files: &[PathBuf]) -> String {
    if recent_files.is_empty() {
        return String::new();
    }

    let mut html = String::new();
    for (index, path) in recent_files.iter().enumerate() {
        let display = path
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|| path.to_string_lossy());
        html.push_str(&format!(
            "<li data-action=\"recent-file\" data-recent-index=\"{index}\">{}</li>",
            escape_html(&display),
        ));
    }
    html
}

fn current_file_name<'a>(state: &'a ViewerState, recent_files: &[PathBuf]) -> &'a str {
    if let Some(document) = state.current_document() {
        return document.file_name.as_str();
    }

    if recent_files.is_empty() {
        ""
    } else {
        "Right-click to open recent files"
    }
}

fn current_document_base_href(state: &ViewerState) -> String {
    let Some(document) = state.current_document() else {
        return String::new();
    };

    let Some(mut href) = file_url_from_path(&document.base_dir) else {
        return String::new();
    };

    if !href.ends_with('/') {
        href.push('/');
    }

    format!("<base href=\"{}\">", escape_html(&href))
}

fn body_font_css(body_font: Option<&BodyFontSettings>) -> String {
    match body_font {
        Some(settings) => {
            let family = css_font_family(&settings.family_name);
            let size_whole = settings.point_size_tenths / 10;
            let size_frac = settings.point_size_tenths % 10;
            let size_str = if size_frac == 0 {
                format!("{}pt", size_whole)
            } else {
                format!("{}.{}pt", size_whole, size_frac)
            };
            format!(
                ".markdown-selection-host {{ --body-font-family: {}; --body-font-size: {}; }}",
                family, size_str
            )
        }
        None => String::new(),
    }
}

fn css_font_family(name: &str) -> String {
    if name.contains(' ') {
        format!("\"{}\", \"Segoe UI\", sans-serif", name)
    } else {
        format!("{}, \"Segoe UI\", sans-serif", name)
    }
}

fn content_html(state: &ViewerState) -> String {
    if matches!(state, ViewerState::ErrorVisible { .. }) && state.current_document().is_none() {
        return String::new();
    }

    let content = state
        .current_document()
        .map(|document| document.html_body.as_str())
        .unwrap_or("<p class=\"empty-state\">Open Markdown file to start reading.</p>");

    let base_dir = state
        .current_document()
        .map(|document| document.base_dir.as_path());
    let content = sanitize_body_html(content, base_dir);

    if state.current_document().is_some() {
        format!(
            "<section class=\"markdown-selection-host\" data-markdown-selection-host data-document-loaded=\"true\" selectable><article class=\"markdown-body\" data-markdown-body data-viewer-mode=\"read-only\">{content}</article></section>"
        )
    } else {
        content
    }
}

fn error_html(state: &ViewerState) -> String {
    match state {
        ViewerState::ErrorVisible { error, .. } if state.current_document().is_none() => format!(
            "<p class=\"error-message\">{}</p>",
            escape_html(&error.user_message())
        ),
        _ => String::new(),
    }
}

fn error_overlay_html(state: &ViewerState) -> String {
    let ViewerState::ErrorVisible { error, .. } = state else {
        return String::new();
    };
    if state.current_document().is_none() {
        return String::new();
    }

    format!(
        concat!(
            "<div class=\"error-overlay\" data-error-overlay>",
            "<section class=\"error-dialog\">",
            "<header><strong>Error</strong></header>",
            "<p>{}</p>",
            "<footer><button data-action=\"error-ok\">OK</button></footer>",
            "</section></div>"
        ),
        escape_html(&error.user_message())
    )
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::{DefaultHtmlShell, HtmlShell, ShellModel};
    use crate::settings::BodyFontSettings;
    use crate::ui::{EmbeddedUiAssets, Theme, UiTextAsset};
    use crate::{RenderedDocument, ViewerError, ViewerState, APP_NAME};
    use std::path::PathBuf;

    fn render_test_shell(state: &ViewerState) -> String {
        render_test_shell_with(state, None, &[], "render shell")
    }

    fn render_test_shell_with(
        state: &ViewerState,
        body_font: Option<&BodyFontSettings>,
        recent_files: &[PathBuf],
        expect_message: &str,
    ) -> String {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());
        shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state,
                theme: Theme::default(),
                body_font,
                recent_files,
            })
            .expect(expect_message)
    }

    #[test]
    fn initial_shell_contains_viewer_identity_open_affordance_and_disabled_future_controls() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(html.contains("<!doctype html>"));
        assert!(html.contains("<html theme=\"light\" window-frame=\"extended\""));
        assert!(html.contains("MDLuma"));
        assert!(html.contains("data-role=\"titlebar-brand\""));
        assert!(html.contains("data-action=\"open-file\""));
        assert!(html.contains("data-role=\"future-controls\""));
        assert!(html.contains("Open Markdown file"));
        assert!(html.contains("data-action=\"window-minimize\""));
        assert!(html.contains("data-action=\"window-toggle-maximize\""));
        assert!(html.contains("data-action=\"window-close\""));
        assert!(html.contains("data-icon-restore-light"));
        assert!(html.contains("data-icon-restore-dark"));
        assert!(html.contains("data-icon-maximize-light"));
        assert!(html.contains("data-icon-maximize-dark"));
        assert!(html.contains("role=\"window-minimize\""));
        assert!(html.contains("role=\"window-maximize\""));
        assert!(html.contains("role=\"window-close\""));
        assert!(html.contains("data-role=\"window-controls\""));
        assert!(html.contains("data-role=\"viewer-shell\""));
        assert!(html.contains("data-role=\"viewer-viewport\""));
        assert!(html.contains("data-role=\"viewport-body\""));
        assert!(html.contains("M18 32h28"));
        assert!(html.contains("rx=%222%22"));
        assert!(html.contains("M20 20l24 24M44 20L20 44"));
        assert!(html.contains("data-content-area"));
        assert!(html.contains("data-error-area"));
        assert!(html.contains("class=\"titlebar-spacer\" aria-hidden=\"true\""));
        assert!(html.contains("data-action=\"search\""));
        assert!(html.contains("data-action=\"theme\""));
        assert!(html.contains("data-action=\"more\""));
        assert!(!html.contains("data-action=\"more\" disabled"));
        assert!(html.contains("data-action=\"font\""));
        assert!(html.contains("<menu.popup>"));
        assert!(html.contains("type=\"menu\""));
        assert!(html.contains("data-current-file"));
        assert!(html.contains("<div class=\"file-name\" data-current-file></div>"));
        assert!(!html.contains("No file open"));
        assert!(!html.contains("Right-click to open recent files"));
        assert!(html.contains("<header class=\"titlebar\">"));
        assert!(html.contains("<div class=\"titlebar-drag-region\" role=\"window-caption\">"));
        assert_eq!(occurrences(&html, "<header class=\"titlebar\""), 1);
        assert_eq!(occurrences(&html, "<div class=\"titlebar-spacer\""), 1);
        assert_eq!(occurrences(&html, "data-role=\"viewer-viewport\""), 1);
        assert!(html.contains("open-file-requested"));
        assert!(!html.contains("search-requested"));
        assert!(!html.contains("save-requested"));
        assert!(html.contains("theme-toggle-requested"));
        assert!(!html.contains("multi-tab-requested"));
        assert!(!html.to_ascii_lowercase().contains("save"));
        assert!(html.contains(r#"data-action="external-editor" disabled"#));
        assert!(
            html.contains(r#"data-action="external-editor-setting""#),
            "External Editor Setting must always be present in the more menu"
        );
        assert!(
            !html.contains(r#"data-action="external-editor-setting" disabled"#),
            "External Editor Setting must never be disabled"
        );
        assert!(!html.to_ascii_lowercase().contains("contenteditable"));
    }

    #[test]
    fn initial_shell_prompts_recent_file_menu_when_recent_files_exist() {
        let recent_files = vec![PathBuf::from(r"C:\docs\guide.md")];
        let html = render_test_shell_with(
            &ViewerState::NoDocument,
            None,
            &recent_files,
            "render initial shell with recent files",
        );

        assert!(html.contains("data-current-file>Right-click to open recent files<"));
        assert!(!html.contains("No file open"));
        assert!(html.contains(r#"data-action="recent-file" data-recent-index="0""#));
    }

    #[test]
    fn document_shell_displays_current_file_name_and_rendered_html_body_readonly() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: "<h1>Guide</h1><p>Read only.</p>".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("guide.md"));
        assert!(html.contains("<base href=\"file:///C:/docs/\">"));
        assert!(html.contains("data-current-file>guide.md<"));
        assert!(html.contains("<div class=\"titlebar-drag-region\" role=\"window-caption\">"));
        assert!(html.contains("<h1>Guide</h1><p>Read only.</p>"));
        assert!(html.contains("<section class=\"viewer-viewport\" data-role=\"viewer-viewport\">"));
        assert!(html.contains("<div class=\"viewport-body\" data-role=\"viewport-body\">"));
        assert!(html.contains(
            "<section class=\"copy-status\" data-copy-status aria-live=\"polite\"></section>"
        ));
        assert!(html.contains("<main class=\"content\" data-content-area data-markdown-body-host><section class=\"markdown-selection-host\" data-markdown-selection-host data-document-loaded=\"true\" selectable><article class=\"markdown-body\" data-markdown-body data-viewer-mode=\"read-only\"><h1>Guide</h1><p>Read only.</p></article></section></main>"));
        assert_eq!(occurrences(&html, "<header class=\"titlebar\""), 1);
        assert_eq!(occurrences(&html, "<div class=\"titlebar-spacer\""), 1);
        assert_eq!(occurrences(&html, "data-role=\"viewer-viewport\""), 1);
        assert!(!html.contains("<textarea"));
        assert!(html.contains("<input type=\"text\" class=\"search-input\""));
    }

    #[test]
    fn document_shell_marks_markdown_selection_host_as_selection_boundary_and_keeps_article_read_only(
    ) {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\selection.md"),
            file_name: "selection.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: "<p>Select me.</p>".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains(
            "<section class=\"markdown-selection-host\" data-markdown-selection-host data-document-loaded=\"true\" selectable><article class=\"markdown-body\" data-markdown-body data-viewer-mode=\"read-only\">"
        ));
    }

    #[test]
    fn document_shell_wraps_rendered_markdown_with_readable_gfm_article_styles() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\gfm.md"),
            file_name: "gfm.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: concat!(
                "<h1>Guide</h1>",
                "<table><thead><tr><th>Column</th></tr></thead><tbody><tr><td>Value</td></tr></tbody></table>",
                "<pre><code>fn main() {}</code></pre>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains(
            "<section class=\"markdown-selection-host\" data-markdown-selection-host data-document-loaded=\"true\" selectable><article class=\"markdown-body\" data-markdown-body data-viewer-mode=\"read-only\"><h1>Guide</h1>"
        ));
        assert!(html.contains(".markdown-body table"));
        assert!(html.contains(".markdown-body pre"));
        assert!(html.contains(".markdown-body code"));
        assert!(!html.contains("# Guide"));
    }

    #[test]
    fn error_shell_renders_user_error_without_losing_previous_file_name() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\existing.md"),
            file_name: "existing.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: "<p>Existing</p>".to_string(),
        })
        .with_error(ViewerError::file_read(r"C:\\docs\\missing.md", "missing"));
        let html = render_test_shell(&state);

        assert!(html.contains("existing.md"));
        assert!(html.contains("MDLuma could not read the selected Markdown file."));
        assert!(html.contains("data-error-overlay"));
        assert!(html.contains(r#"data-action="error-ok">OK<"#));
        assert!(!html
            .contains(r#"<section class="error-area" data-error-area><p class="error-message">"#));
        assert_eq!(occurrences(&html, "<header class=\"titlebar\""), 1);
        assert_eq!(occurrences(&html, "<div class=\"titlebar-spacer\""), 1);
        assert_eq!(occurrences(&html, "data-role=\"viewer-viewport\""), 1);
    }

    #[test]
    fn error_shell_without_previous_document_keeps_single_top_bar_and_replaces_empty_prompt() {
        let state = ViewerState::NoDocument
            .with_error(ViewerError::file_read(r"C:\\docs\\missing.md", "missing"));
        let html = render_test_shell(&state);

        assert!(html.contains("<div class=\"file-name\" data-current-file></div>"));
        assert!(!html.contains("No file open"));
        assert!(html.contains("MDLuma could not read the selected Markdown file."));
        assert!(html.contains("<section class=\"error-area\" data-error-area><p class=\"error-message\">MDLuma could not read the selected Markdown file.</p></section>"));
        assert_eq!(occurrences(&html, "<header class=\"titlebar\""), 1);
        assert_eq!(occurrences(&html, "<div class=\"titlebar-spacer\""), 1);
        assert_eq!(occurrences(&html, "data-role=\"viewer-viewport\""), 1);
        assert!(!html.contains("Open Markdown file to start reading."));
    }

    #[test]
    fn local_only_shell_sanitizes_remote_links_and_images_from_rendered_body() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\remote.md"),
            file_name: "remote.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: concat!(
                "<p><a href=\"https://example.com/doc\">Remote link</a></p>",
                "<img src=\"http://example.com/image.png\" alt=\"demo\">"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("Remote link"));
        assert!(html.contains("href=\"#\""));
        assert!(html.contains("data-href=\"https://example.com/doc\""));
        assert!(html.contains("src=\"\""));
        assert!(!html.contains("<a href=\"https://"));
        assert!(!html.contains("http://example.com/image.png"));
    }

    #[test]
    fn local_only_shell_rewrites_relative_image_src_to_absolute_file_url() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\\docs\\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            html_body: "<p><img src=\"images/hero.jpg\" alt=\"hero\"></p>".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("src=\"file:///C:/docs/images/hero.jpg\""));
    }

    #[test]
    fn local_only_shell_sanitizes_unquoted_remote_resource_attributes() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\unquoted-remote.md"),
            file_name: "unquoted-remote.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                "<p><a href=https://example.com/doc>Remote link</a></p>",
                "<img src=https://example.com/image.png alt=demo>",
                "<form action=//example.com/upload></form>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("href=#"));
        assert!(html.contains("src="));
        assert!(!html.contains("href=https://example.com/doc"));
        assert!(!html.contains("src=https://example.com/image.png"));
        assert!(!html.contains("<form"));
        assert!(html.contains("&lt;form action=//example.com/upload&gt;"));
        assert!(html.contains("data-href=\"https://example.com/doc\""));
        assert!(!html.contains("https://example.com/image.png"));
    }

    #[test]
    fn initial_shell_has_no_base_href_tag_when_no_document_is_loaded() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(!html.contains("<base href="));
    }

    #[test]
    #[cfg(windows)]
    fn document_shell_normalizes_verbatim_windows_path_for_base_href() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"\\?\C:\docs\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"\\?\C:\docs"),
            html_body: "<h1>Guide</h1>".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("<base href=\"file:///C:/docs/\">"));
    }

    #[test]
    fn local_only_shell_sanitizes_mixed_case_remote_attributes_without_touching_text_content() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\mixed-case-remote.md"),
            file_name: "mixed-case-remote.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                "<p><a HREF=\"https://example.com/doc\">Remote link</a></p>",
                "<img SRC=https://example.com/image.png alt=demo>",
                "<pre><code>literal HREF=https://example.com/doc should stay visible</code></pre>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("HREF=\"#\""));
        assert!(html.contains("SRC="));
        assert!(!html.contains("HREF=\"https://example.com/doc\""));
        assert!(!html.contains("SRC=https://example.com/image.png"));
        assert!(html.contains("literal HREF=https://example.com/doc should stay visible"));
    }

    #[test]
    fn local_only_shell_removes_event_attributes_and_escapes_disallowed_embeds() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\unsafe-html.md"),
            file_name: "unsafe-html.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                "<p><span onclick=\"alert(1)\" data-note=\"safe\">Click me</span></p>",
                "<iframe src=\"https://example.com/embed\"></iframe>",
                "<script>alert(1)</script>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("<span>Click me</span>"));
        assert!(!html.contains("data-note="));
        assert!(!html.to_ascii_lowercase().contains("onclick="));
        assert!(!html.contains("<iframe"));
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;iframe"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn local_only_shell_blocks_dangerous_url_schemes_but_keeps_safe_semantic_html() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\allowed-and-blocked.md"),
            file_name: "allowed-and-blocked.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                "<details open><summary>Shortcuts</summary><p><kbd>Ctrl</kbd> + <kbd>K</kbd></p></details>",
                "<p><a href=\"java\nscript:alert(1)\">bad</a> <a href=\"FiLe:///C:/docs/guide.md\">file</a> <a href=\"#local-fragment\">fragment</a></p>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("<details open>"));
        assert!(html.contains("<summary>Shortcuts</summary>"));
        assert!(html.contains("<kbd>Ctrl</kbd> + <kbd>K</kbd>"));
        assert!(html.contains("href=\"#\""));
        assert!(html.contains("href=\"#local-fragment\""));
        assert!(!html.contains("java\nscript:alert(1)"));
        assert!(!html.contains("FiLe:///C:/docs/guide.md"));
    }

    #[test]
    fn local_only_shell_clears_srcset_when_any_candidate_is_remote() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\remote-srcset.md"),
            file_name: "remote-srcset.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<img srcset=\"cover.png 1x, https://example.com/cover@2x.png 2x\" src=\"cover.png\" alt=\"cover\">".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(!html.contains("srcset="));
        assert!(html.contains("src=\"file:///C:/docs/cover.png\""));
        assert!(!html.contains("https://example.com/cover@2x.png"));
    }

    #[test]
    fn local_only_shell_keeps_raw_html_checkboxes_visible_but_forces_disabled() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\raw-checkbox.md"),
            file_name: "raw-checkbox.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                "<p>Done <input type=\"checkbox\" checked></p>",
                "<p>Todo <input type=\"checkbox\"></p>"
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("Done <input type=\"checkbox\" checked disabled>"));
        assert!(html.contains("Todo <input type=\"checkbox\" disabled>"));
        assert!(!html.contains("<input type=\"checkbox\" checked>"));
        assert!(!html.contains("<input type=\"checkbox\">"));
    }

    #[test]
    fn shell_does_not_reference_remote_or_network_resources() {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());

        let html = shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state: &ViewerState::NoDocument,
                theme: Theme::default(),
                body_font: None,
                recent_files: &[],
            })
            .expect("render local-only shell");

        let lower = html.to_ascii_lowercase();
        let lower = lower.replace("http://sciter.com/", "");
        for forbidden in ["http://", "https://", "ftp://", "//cdn", "javascript:"] {
            assert!(
                !lower.contains(forbidden),
                "unexpected remote reference: {forbidden}"
            );
        }
        assert!(
            lower.contains("data:image/svg+xml"),
            "shell should embed icons as data URLs"
        );
    }

    #[test]
    fn embedded_assets_contain_no_network_references() {
        let assets = EmbeddedUiAssets::default();
        let template = assets
            .read_text_asset(UiTextAsset::IndexHtml)
            .expect("read index.html");
        let styles = assets
            .read_text_asset(UiTextAsset::StylesCss)
            .expect("read styles.css");
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app.js");

        for (name, content) in [
            ("index.html", template.as_ref()),
            ("styles.css", styles.as_ref()),
            ("app.js", script.as_ref()),
        ] {
            let lower = content.to_ascii_lowercase();
            let lower = lower.replace("http://sciter.com/", "");
            for forbidden in ["http://", "https://", "ftp://", "//cdn"] {
                assert!(
                    !lower.contains(forbidden),
                    "{name}: unexpected remote reference: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn dark_theme_shell_contains_theme_dark_and_dark_icon_colors() {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());

        let html = shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state: &ViewerState::NoDocument,
                theme: Theme::Dark,
                body_font: None,
                recent_files: &[],
            })
            .expect("render dark shell");

        assert!(html.contains("<html theme=\"dark\" window-frame=\"extended\""));
        assert!(html.contains("data-icon-light"));
        assert!(html.contains("data-icon-dark"));
        assert!(html.contains("fill=%22%23D1D5DB%22"));
        assert!(html.contains("fill=%22%23111827%22"));
    }

    #[test]
    fn light_theme_shell_contains_theme_light_and_light_icon_colors() {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());

        let html = shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state: &ViewerState::NoDocument,
                theme: Theme::Light,
                body_font: None,
                recent_files: &[],
            })
            .expect("render light shell");

        assert!(html.contains("<html theme=\"light\" window-frame=\"extended\""));
        assert!(html.contains("data-icon-light"));
        assert!(html.contains("data-icon-dark"));
        assert!(html.contains("fill=%22%23111827%22"));
        assert!(html.contains("fill=%22%23D1D5DB%22"));
    }

    #[test]
    fn dark_theme_toggle_icon_is_sun() {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());

        let html = shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state: &ViewerState::NoDocument,
                theme: Theme::Dark,
                body_font: None,
                recent_files: &[],
            })
            .expect("render dark shell");

        assert!(html.contains("cx=%2232%22 cy=%2232%22"));
    }

    #[test]
    fn light_theme_toggle_icon_is_moon() {
        let shell = DefaultHtmlShell::new(EmbeddedUiAssets::default());

        let html = shell
            .render_shell(ShellModel {
                app_name: APP_NAME,
                state: &ViewerState::NoDocument,
                theme: Theme::Light,
                body_font: None,
                recent_files: &[],
            })
            .expect("render light shell");

        assert!(html.contains("M41.8 13.5"));
    }

    #[test]
    fn body_font_none_does_not_inject_css_variables() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(!html.contains("--body-font-family:"));
        assert!(!html.contains("--body-font-size:"));
    }

    #[test]
    fn body_font_some_injects_css_variables_on_markdown_body() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\font.md"),
            file_name: "font.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Hello</p>".to_string(),
        });
        let body_font = BodyFontSettings {
            family_name: "Yu Gothic UI".to_string(),
            point_size_tenths: 120,
        };
        let html =
            render_test_shell_with(&state, Some(&body_font), &[], "render shell with body font");

        assert!(html.contains("--body-font-family"));
        assert!(html.contains("--body-font-size"));
        assert!(html.contains("Yu Gothic UI"));
        assert!(html.contains("12pt"));
    }

    #[test]
    fn body_font_css_is_scoped_to_markdown_selection_host() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\scoped.md"),
            file_name: "scoped.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Scoped</p>".to_string(),
        });
        let body_font = BodyFontSettings {
            family_name: "Consolas".to_string(),
            point_size_tenths: 140,
        };
        let html = render_test_shell_with(
            &state,
            Some(&body_font),
            &[],
            "render shell with scoped body font",
        );

        assert!(html.contains(".markdown-selection-host"));
        assert!(html.contains("Consolas"));
        assert!(html.contains("14pt"));
    }

    #[test]
    fn body_font_family_with_spaces_is_css_quoted() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\escape.md"),
            file_name: "escape.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Escaped</p>".to_string(),
        });
        let body_font = BodyFontSettings {
            family_name: "Font With Spaces".to_string(),
            point_size_tenths: 110,
        };
        let html = render_test_shell_with(
            &state,
            Some(&body_font),
            &[],
            "render shell with quoted body font",
        );

        assert!(html.contains("\"Font With Spaces\""));
        assert!(html.contains("Segoe UI"));
        assert!(html.contains("sans-serif"));
        assert!(html.contains("11pt"));
    }

    #[test]
    fn body_font_includes_fallback_chain_after_selected_font() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\fallback.md"),
            file_name: "fallback.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Fallback</p>".to_string(),
        });
        let body_font = BodyFontSettings {
            family_name: "CustomFont".to_string(),
            point_size_tenths: 100,
        };
        let html = render_test_shell_with(
            &state,
            Some(&body_font),
            &[],
            "render shell with fallback chain",
        );

        let family_idx = html.find("CustomFont").expect("font name present");
        let remainder = &html[family_idx..];
        let fallback_idx = remainder.find("Segoe UI").expect("fallback present");
        let generic_idx = remainder.find("sans-serif").expect("generic present");
        assert!(fallback_idx < generic_idx);
    }

    #[test]
    fn no_document_shell_has_external_editor_disabled() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(html.contains("data-action=\"external-editor\""));
        assert!(html.contains("data-action=\"external-editor\" disabled"));
        assert!(!html.contains("data-document-loaded=\"true\""));
        assert!(!html.to_ascii_lowercase().contains("contenteditable"));
    }

    #[test]
    fn document_loaded_shell_has_external_editor_enabled() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Hello</p>".to_string(),
        });
        let html = render_test_shell(&state);

        assert!(html.contains("data-action=\"external-editor\""));
        assert!(!html.contains("data-action=\"external-editor\" disabled"));
        assert!(html.contains("data-document-loaded=\"true\""));
    }

    #[test]
    fn error_visible_with_previous_document_has_external_editor_enabled() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\existing.md"),
            file_name: "existing.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Existing</p>".to_string(),
        })
        .with_error(ViewerError::file_read(r"C:\docs\missing.md", "missing"));
        let html = render_test_shell(&state);

        assert!(html.contains("data-action=\"external-editor\""));
        assert!(!html.contains("data-action=\"external-editor\" disabled"));
        assert!(html.contains("data-document-loaded=\"true\""));
    }

    #[test]
    fn error_visible_without_previous_document_has_external_editor_disabled() {
        let state = ViewerState::NoDocument
            .with_error(ViewerError::file_read(r"C:\docs\missing.md", "missing"));
        let html = render_test_shell(&state);

        assert!(html.contains("data-action=\"external-editor\""));
        assert!(html.contains("data-action=\"external-editor\" disabled"));
    }

    #[test]
    fn external_editor_appears_after_font_in_menu() {
        let html = render_test_shell(&ViewerState::NoDocument);

        let menu_start = html.find("<menu.popup>").expect("more menu popup present");
        let menu_html = &html[menu_start..];

        let font_pos = menu_html
            .find(r#"data-action="font""#)
            .expect("font action present");
        let editor_pos = menu_html
            .find(r#"data-action="external-editor""#)
            .expect("external-editor action present");
        assert!(
            editor_pos > font_pos,
            "External Editor must appear after Font"
        );
    }

    #[test]
    fn version_substitution_replaces_placeholder_with_cargo_pkg_version() {
        let input = "before {{VERSION}} after";
        let result = input.replace("{{VERSION}}", env!("CARGO_PKG_VERSION"));
        assert!(
            !result.contains("{{VERSION}}"),
            "{{VERSION}} should be substituted"
        );
        assert!(
            result.contains(env!("CARGO_PKG_VERSION")),
            "result should contain CARGO_PKG_VERSION"
        );
    }

    #[test]
    fn build_number_substitution_replaces_placeholder_with_git_commit_hash() {
        let input = "before {{BUILD_NUMBER}} after";
        let result = input.replace("{{BUILD_NUMBER}}", env!("GIT_COMMIT_HASH"));
        assert!(
            !result.contains("{{BUILD_NUMBER}}"),
            "{{BUILD_NUMBER}} should be substituted"
        );
        assert!(
            result.contains(env!("GIT_COMMIT_HASH")),
            "result should contain GIT_COMMIT_HASH"
        );
    }

    #[test]
    fn version_and_build_number_substitutions_coexist_with_existing_placeholders() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(
            html.contains("MDLuma"),
            "APP_NAME substitution should still work"
        );
        assert!(
            !html.contains("{{VERSION}}"),
            "{{VERSION}} should not leak into output if template has it"
        );
        assert!(
            !html.contains("{{BUILD_NUMBER}}"),
            "{{BUILD_NUMBER}} should not leak into output if template has it"
        );
    }

    #[test]
    fn about_menu_item_appears_at_end_of_more_menu() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(html.contains(r#"data-action="about">About<"#));

        let menu_start = html.find("<menu.popup>").expect("more menu popup present");
        let menu_html = &html[menu_start..];
        let menu_end = menu_html.find("</menu>").expect("menu closing tag present");
        let menu_content = &menu_html[..menu_end];

        let about_pos = menu_content
            .find(r#"data-action="about""#)
            .expect("about action present");
        let editor_pos = menu_content
            .find(r#"data-action="external-editor""#)
            .expect("external-editor action present");
        assert!(
            about_pos > editor_pos,
            "About must appear after External Editor"
        );
    }

    #[test]
    fn external_editor_setting_appears_between_external_editor_and_about_in_menu() {
        let html = render_test_shell(&ViewerState::NoDocument);

        let menu_start = html.find("<menu.popup>").expect("more menu popup present");
        let menu_html = &html[menu_start..];
        let menu_end = menu_html.find("</menu>").expect("menu closing tag present");
        let menu_content = &menu_html[..menu_end];

        let font_pos = menu_content
            .find(r#"data-action="font""#)
            .expect("font action present");
        let editor_pos = menu_content
            .find(r#"data-action="external-editor""#)
            .expect("external-editor action present");
        let setting_pos = menu_content
            .find(r#"data-action="external-editor-setting""#)
            .expect("external-editor-setting action present");
        let about_pos = menu_content
            .find(r#"data-action="about""#)
            .expect("about action present");

        assert!(
            editor_pos > font_pos,
            "External Editor must appear after Font"
        );
        assert!(
            setting_pos > editor_pos,
            "External Editor Setting must appear after External Editor"
        );
        assert!(
            about_pos > setting_pos,
            "About must appear after External Editor Setting"
        );
    }

    #[test]
    fn external_editor_setting_never_disabled_regardless_of_document() {
        let no_doc_html = render_test_shell(&ViewerState::NoDocument);

        assert!(
            no_doc_html.contains(r#"data-action="external-editor-setting""#),
            "setting item must be present when no document"
        );
        assert!(
            !no_doc_html.contains(r#"data-action="external-editor-setting" disabled"#),
            "setting item must not be disabled when no document"
        );
        assert!(
            no_doc_html.contains(r#"data-action="external-editor" disabled"#),
            "external editor must be disabled when no document"
        );

        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\guide.md"),
            file_name: "guide.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Hello</p>".to_string(),
        });
        let doc_html = render_test_shell(&state);

        assert!(
            doc_html.contains(r#"data-action="external-editor-setting""#),
            "setting item must be present when document loaded"
        );
        assert!(
            !doc_html.contains(r#"data-action="external-editor-setting" disabled"#),
            "setting item must not be disabled when document loaded"
        );
        assert!(
            !doc_html.contains(r#"data-action="external-editor" disabled"#),
            "external editor must be enabled when document loaded"
        );
    }

    #[test]
    fn about_dialog_element_exists_with_required_content() {
        let html = render_test_shell(&ViewerState::NoDocument);

        assert!(html.contains(r#"data-about-overlay"#));
        assert!(html.contains(r#"data-about-overlay hidden"#));
        assert!(html.contains(r#"data-action="about-ok">OK<"#));
        assert!(html.contains("about-dialog"));
        assert!(html.contains("MDLuma"));
        assert!(html.contains(env!("CARGO_PKG_VERSION")));
        assert!(html.contains(env!("GIT_COMMIT_HASH")));
        assert!(html.contains("Akira Shimosako"));
        assert!(html.contains("Licensed under MIT OR Apache-2.0."));
        assert!(html.contains("Terra Informatica"));
        assert!(html.contains("sciter.com"));
    }

    #[test]
    fn error_overlay_exists_only_when_document_remains_visible() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\existing.md"),
            file_name: "existing.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: "<p>Existing</p>".to_string(),
        })
        .with_error(ViewerError::file_read(r"C:\docs\missing.md", "missing"));

        let html = render_test_shell(&state);

        assert!(html.contains("error-dialog"));
        assert!(html.contains("data-error-overlay"));
        assert!(html.contains(r#"data-action="error-ok">OK<"#));
        assert!(html.contains("<p>Existing</p>"));
    }

    fn occurrences(haystack: &str, needle: &str) -> usize {
        haystack.match_indices(needle).count()
    }

    #[test]
    fn data_href_preserves_http_and_https_urls_but_not_other_schemes() {
        let state = ViewerState::DocumentLoaded(RenderedDocument {
            path: PathBuf::from(r"C:\docs\links.md"),
            file_name: "links.md".to_string(),
            base_dir: PathBuf::from(r"C:\docs"),
            html_body: concat!(
                r#"<a href="https://example.com/safe">https</a>"#,
                r#" <a href="http://example.com/plain">http</a>"#,
                r#" <a href="ftp://example.com/file">ftp</a>"#,
                r#" <a href="mailto:test@example.com">mail</a>"#,
                r##" <a href="#fragment">frag</a>"##,
            )
            .to_string(),
        });
        let html = render_test_shell(&state);

        assert!(
            html.contains(r#"data-href="https://example.com/safe""#),
            "https URL should be preserved in data-href"
        );
        assert!(
            html.contains(r#"data-href="http://example.com/plain""#),
            "http URL should be preserved in data-href"
        );
        assert!(
            !html.contains("data-href=\"ftp://"),
            "ftp URL should not be preserved in data-href"
        );
        assert!(
            !html.contains("data-href=\"mailto:"),
            "mailto URL should not be preserved in data-href"
        );
        assert!(
            !html.contains("data-href=\"#fragment\""),
            "fragment link should not get data-href"
        );
        assert!(
            html.contains(r##"href="#fragment""##),
            "fragment href should be kept unchanged"
        );
    }
}

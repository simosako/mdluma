use crate::ViewerError;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTextAsset {
    IndexHtml,
    StylesCss,
    AppJs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconTheme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconName {
    App,
    Open,
    Search,
    SearchPrev,
    SearchNext,
    SearchClose,
    More,
    Sun,
    Moon,
    WindowMinimize,
    WindowMaximize,
    WindowRestore,
    WindowClose,
}

macro_rules! embedded_svg_match {
    ($name:expr, $theme:expr, { $($variant:ident => $file_stem:literal),+ $(,)? }) => {
        match ($name, $theme) {
            $(
                (IconName::$variant, IconTheme::Light) => {
                    include_str!(concat!("../../assets/light/", $file_stem, ".svg"))
                }
                (IconName::$variant, IconTheme::Dark) => {
                    include_str!(concat!("../../assets/dark/", $file_stem, ".svg"))
                }
            )+
        }
    };
}

impl IconName {
    fn embedded_svg(self, theme: IconTheme) -> &'static str {
        embedded_svg_match!(self, theme, {
            App => "app",
            Open => "open",
            Search => "search",
            SearchPrev => "search-prev",
            SearchNext => "search-next",
            SearchClose => "search-close",
            More => "more",
            Sun => "sun",
            Moon => "moon",
            WindowMinimize => "window-minimize",
            WindowMaximize => "window-maximize",
            WindowRestore => "window-restore",
            WindowClose => "window-close",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Default for Theme {
    fn default() -> Self {
        Self::Light
    }
}

impl Theme {
    pub fn toggle(self) -> Self {
        match self {
            Self::Light => Self::Dark,
            Self::Dark => Self::Light,
        }
    }

    pub fn theme_attr(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn icon_theme(self) -> IconTheme {
        match self {
            Self::Light => IconTheme::Light,
            Self::Dark => IconTheme::Dark,
        }
    }

    pub fn toggle_icon(self) -> IconName {
        match self {
            Self::Light => IconName::Moon,
            Self::Dark => IconName::Sun,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EmbeddedUiAssets;

impl EmbeddedUiAssets {
    pub fn read_text_asset(&self, asset: UiTextAsset) -> Result<Cow<'static, str>, ViewerError> {
        let text = match asset {
            UiTextAsset::IndexHtml => include_str!("index.html"),
            UiTextAsset::StylesCss => include_str!("styles.css"),
            UiTextAsset::AppJs => include_str!("app.js"),
        };

        Ok(Cow::Borrowed(text))
    }

    pub fn icon_data_url(&self, name: IconName, theme: IconTheme) -> Result<String, ViewerError> {
        let svg = name.embedded_svg(theme);
        let stripped = svg.replace(" xmlns=\"http://www.w3.org/2000/svg\"", "");
        let encoded = stripped.replace('#', "%23").replace('"', "%22");
        Ok(format!("data:image/svg+xml;charset=utf-8,{encoded}"))
    }
}

#[cfg(test)]
mod tests {
    use super::{EmbeddedUiAssets, IconName, IconTheme, Theme, UiTextAsset};
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn theme_default_is_light() {
        assert_eq!(Theme::default(), Theme::Light);
    }

    #[test]
    fn theme_toggle_swaps_light_and_dark() {
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
    }

    #[test]
    fn theme_toggle_round_trips() {
        assert_eq!(Theme::Light.toggle().toggle(), Theme::Light);
        assert_eq!(Theme::Dark.toggle().toggle(), Theme::Dark);
    }

    #[test]
    fn theme_attr_returns_correct_strings() {
        assert_eq!(Theme::Light.theme_attr(), "light");
        assert_eq!(Theme::Dark.theme_attr(), "dark");
    }

    #[test]
    fn theme_icon_theme_maps_correctly() {
        assert_eq!(Theme::Light.icon_theme(), IconTheme::Light);
        assert_eq!(Theme::Dark.icon_theme(), IconTheme::Dark);
    }

    #[test]
    fn theme_toggle_icon_returns_opposite_icon() {
        assert_eq!(Theme::Light.toggle_icon(), IconName::Moon);
        assert_eq!(Theme::Dark.toggle_icon(), IconName::Sun);
    }

    #[test]
    fn icon_name_sun_and_moon_are_defined_variants() {
        let _ = IconName::Sun;
        let _ = IconName::Moon;
    }

    #[test]
    fn embedded_text_assets_are_local_shell_resources() {
        let assets = EmbeddedUiAssets::default();

        let index = assets
            .read_text_asset(UiTextAsset::IndexHtml)
            .expect("read index html");
        let css = assets
            .read_text_asset(UiTextAsset::StylesCss)
            .expect("read styles css");
        let js = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        assert!(index.contains("{{APP_NAME}}"));
        assert!(css.contains(".titlebar"));
        assert!(css.contains(".content"));
        assert!(css.contains(".markdown-body"));
        assert!(css.contains(".markdown-body table"));
        assert!(css.contains(".markdown-body pre"));
        assert!(css.contains("width: *"));
        assert!(css.contains("max-width: 1040px"));
        assert!(css.contains("margin: 0 auto"));
        assert!(!css.contains("max-width: 1100px"));
        assert!(js.contains("open-file-requested"));
        assert!(js.contains("requestOpenFile"));
        assert!(js.contains("Window.this.xcall"));
        assert!(!js.contains("search-requested"));
        assert!(!js.contains("save-requested"));
        assert!(js.contains("theme-toggle-requested"));
        assert!(!js.contains("multi-tab-requested"));

        for asset in [index, css, js] {
            let lower = asset.to_ascii_lowercase();
            assert!(!lower.contains("http://"));
            assert!(!lower.contains("https://"));
            assert!(!lower.contains("@import"));
        }
    }

    #[test]
    fn embedded_assets_expose_titlebar_interaction_contract() {
        let assets = EmbeddedUiAssets::default();

        let index = assets
            .read_text_asset(UiTextAsset::IndexHtml)
            .expect("read index html");
        let js = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        assert!(index.contains("data-action=\"open-file\""));
        assert!(index.contains("window-frame=\"extended\""));
        assert!(index.contains("data-action=\"window-minimize\""));
        assert!(index.contains("data-action=\"window-toggle-maximize\""));
        assert!(index.contains("data-action=\"window-close\""));
        assert!(index.contains("role=\"window-minimize\""));
        assert!(index.contains("role=\"window-maximize\""));
        assert!(index.contains("role=\"window-close\""));
        assert!(index.contains("role=\"window-caption\""));

        assert!(js.contains("open-file-requested"));
        assert!(js.contains("xcall"));
        assert!(js.contains("target.disabled"));
        assert!(!js.contains("search-requested"));
        assert!(js.contains("theme-toggle-requested"));
        assert!(!js.contains("save-requested"));
    }

    #[test]
    fn embedded_index_html_exposes_markdown_body_and_copy_status_anchors() {
        let assets = EmbeddedUiAssets::default();
        let index = assets
            .read_text_asset(UiTextAsset::IndexHtml)
            .expect("read index html");

        assert!(index.contains(
            "<main class=\"content\" data-content-area data-markdown-body-host>{{CONTENT}}</main>"
        ));
        assert!(index.contains(
            "<section class=\"copy-status\" data-copy-status aria-live=\"polite\"></section>"
        ));
        assert!(index.contains("<section class=\"error-area\" data-error-area>{{ERROR}}</section>"));
    }

    #[test]
    fn embedded_styles_preserve_body_only_selection_contract() {
        let assets = EmbeddedUiAssets::default();
        let css = assets
            .read_text_asset(UiTextAsset::StylesCss)
            .expect("read styles css");

        assert!(css.contains(".titlebar {"));
        assert!(css.contains("user-select: none;"));
        assert!(css.contains(".markdown-selection-host[selectable]"));
        assert!(css.contains("user-select: text;"));
        assert!(css.contains(".markdown-body::selection"));
        assert!(css.contains(".markdown-body *::selection"));
        assert!(css.contains(".copy-status {"));
        assert!(css.contains("color: var(--viewer-error);"));
    }

    #[test]
    fn embedded_app_js_exposes_copy_shortcut_contract() {
        let assets = EmbeddedUiAssets::default();
        let js = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        assert!(js.contains("Clipboard.writeText"));
        assert!(js.contains("ctrlKey"));
        assert!(js.contains("metaKey"));
        assert!(js.contains("isModifiedLetterShortcut(event, \"o\")"));
        assert!(js.contains("selection.toString()"));
        assert!(js.contains("[data-markdown-body]"));
        assert!(js.contains("[data-markdown-selection-host]"));
        assert!(js.contains("[data-copy-status]"));
        assert!(js.contains("Window.this.update()"));
        assert!(js.contains("clearCopyStatus"));
        assert!(js.contains("showCopyFailure"));
        assert!(js.contains("contextmenu"));
        assert!(js.contains("menu.className = \"context\""));
        assert!(js.contains("data-document-loaded"));
        assert!(js.contains("data-action=\"copy\""));
        assert!(js.contains("edit:selectall"));
    }

    #[test]
    fn markdown_context_menu_script_marks_external_editor_disabled_without_document() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const markdownHost = {
  querySelector() {
    return null;
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body-host]") {
      return markdownHost;
    }
    return null;
  },
};

eval(scriptSource);

const html = globalThis.__mdlumaTestHooks.markdownContextMenuHtml();
if (!html.includes('data-action="copy"')) {
  throw new Error("context menu must include Copy action");
}
if (!html.includes('name="edit:selectall"')) {
  throw new Error("context menu must include Select All action");
}
if (!html.includes('data-action="external-editor" disabled')) {
  throw new Error("external editor must be disabled without document: " + html);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn markdown_context_menu_script_binds_directly_to_markdown_selection_host() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const contextHandlers = {};
const openButton = {
  addEventListener() {},
};

const loadedMarker = {};
const markdownSelectionHost = {
  addEventListener(type, handler) {
    contextHandlers[type] = handler;
  },
  querySelector(selector) {
    if (selector === "[data-document-loaded]") {
      return loadedMarker;
    }
    return null;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  on(type, handler) {
    listeners[type] = handler;
  },
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-selection-host]") {
      return markdownSelectionHost;
    }
    if (selector === "[data-markdown-body]") {
      return null;
    }
    if (selector === "[data-markdown-body-host]") {
      return markdownSelectionHost;
    }
    return null;
  },
};

eval(scriptSource);

if (typeof contextHandlers.contextmenu !== "function") {
  throw new Error("expected direct contextmenu binding on markdown selection host");
}

if (calls.length !== 0) {
  throw new Error("binding should not trigger xcalls: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn markdown_context_menu_script_handles_markdown_right_click_with_loaded_document() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const loadedMarker = {};
const markdownHost = {
  addEventListener() {},
  querySelector(selector) {
    if (selector === "[data-document-loaded]") {
      return loadedMarker;
    }
    return null;
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  createElement(tag) {
    return {
      tagName: tag,
      className: "",
      innerHTML: "",
    };
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body-host]") {
      return markdownHost;
    }
    return null;
  },
};

eval(scriptSource);

const event = {
  target: {
    closest(selector) {
      if (
        selector === "[data-markdown-body-host]" ||
        selector === "[data-markdown-body]" ||
        selector === "[data-markdown-selection-host]"
      ) {
        return markdownHost;
      }
      return null;
    },
  },
  source: null,
};

const handled = globalThis.__mdlumaTestHooks.handleMarkdownContextMenu(event, event.target);
if (!handled) {
  throw new Error("markdown context menu should be handled");
}
if (!event.source) {
  throw new Error("context menu handler must attach menu source");
}
if (event.source.className !== "context") {
  throw new Error("context menu source must be menu.context compatible");
}
if (!event.source.innerHTML.includes('data-action="external-editor"')) {
  throw new Error("context menu must include external editor action");
}
if (event.source.innerHTML.includes('data-action="external-editor" disabled')) {
  throw new Error("external editor should be enabled when a document is loaded: " + event.source.innerHTML);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn markdown_context_menu_script_handles_loaded_document_child_target() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const loadedMarker = {};
const markdownHost = {
  addEventListener() {},
  querySelector(selector) {
    if (selector === "[data-document-loaded]") {
      return loadedMarker;
    }
    return null;
  },
};

const paragraph = {
  parentElement: markdownHost,
  closest(selector) {
    if (
      selector === "[data-markdown-body-host]" ||
      selector === "[data-markdown-body]" ||
      selector === "[data-markdown-selection-host]"
    ) {
      return markdownHost;
    }
    return null;
  },
};

const textNode = {
  parentElement: paragraph,
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  createElement(tag) {
    return {
      tagName: tag,
      className: "",
      innerHTML: "",
    };
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body-host]") {
      return markdownHost;
    }
    return null;
  },
};

eval(scriptSource);

const event = {
  target: textNode,
  source: null,
};

const handled = globalThis.__mdlumaTestHooks.handleMarkdownContextMenu(event, event.target);
if (!handled) {
  throw new Error("markdown context menu should handle child targets inside a loaded document");
}
if (!event.source) {
  throw new Error("child target handling must still attach a context menu source");
}
if (event.source.innerHTML.includes('data-action="external-editor" disabled')) {
  throw new Error("external editor should stay enabled for child targets inside a loaded document: " + event.source.innerHTML);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn markdown_context_menu_select_all_click_selects_markdown_body_contents() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const markdownBodyElement = {};
let focused = 0;
let removedRanges = 0;
let addedRangeTarget = null;

const markdownSelectionHost = {
  focus() {
    focused += 1;
  },
  selection: {
    removeAllRanges() {
      removedRanges += 1;
    },
    addRange(range) {
      addedRangeTarget = range.target;
    },
  },
};

global.Range = function Range() {
  this.target = null;
};

global.Range.prototype.selectNodeContents = function(node) {
  this.target = node;
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-selection-host]") {
      return markdownSelectionHost;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    return null;
  },
};

eval(scriptSource);

const actionTarget = {
  disabled: false,
  getAttribute(name) {
    if (name === "data-action") {
      return "select-all";
    }
    return null;
  },
  closest(selector) {
    if (selector === "[data-action]") {
      return this;
    }
    return null;
  },
};

globalThis.__mdlumaTestHooks.handleClick(actionTarget);

if (focused !== 1) {
  throw new Error("select all should focus the markdown selection host once, got " + focused);
}
if (removedRanges !== 1) {
  throw new Error("select all should clear existing ranges once, got " + removedRanges);
}
if (addedRangeTarget !== markdownBodyElement) {
  throw new Error("select all should target the markdown body contents");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn titlebar_interaction_script_dispatches_only_supported_commands() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const buttonListeners = {};
const openButton = {
  disabled: false,
  getAttribute(name) {
    if (name === "data-action") {
      return "open-file";
    }
    return null;
  },
  closest(selector) {
    if (selector === "[data-action]") {
      return this;
    }
    return null;
  },
  addEventListener(type, handler) {
    buttonListeners[type] = handler;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const openActionButton = createTarget({ action: "open-file" });
listeners.click({ target: createTarget({ actionTarget: openActionButton }) });
buttonListeners.click({ currentTarget: openActionButton });

const minimizeButton = createTarget({ action: "window-minimize" });
listeners.click({ target: createTarget({ actionTarget: minimizeButton }) });

const maximizeButton = createTarget({ action: "window-toggle-maximize" });
listeners.click({ target: createTarget({ actionTarget: maximizeButton }) });

const closeButton = createTarget({ action: "window-close" });
listeners.click({ target: createTarget({ actionTarget: closeButton }) });

if (JSON.stringify(calls) !== JSON.stringify([
  "open-file-requested",
])) {
  throw new Error("unexpected click commands: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn titlebar_interaction_script_ignores_disabled_controls() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const openButton = {
  addEventListener() {},
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const disabledSearch = createTarget({ action: "search", disabled: true });
listeners.click({ target: createTarget({ actionTarget: disabledSearch }) });

const disabledTheme = createTarget({ action: "theme", disabled: true });
listeners.click({ target: createTarget({ actionTarget: disabledTheme }) });

const closeButton = createTarget({ action: "window-close" });
listeners.click({ target: createTarget({ actionTarget: closeButton }) });

if (calls.length !== 0) {
  throw new Error("unexpected disabled behavior: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn titlebar_interaction_script_sciter_delegate_ignores_non_open_actions() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const delegated = {};
const direct = {};
const openButton = {
  addEventListener(type, handler) {
    direct[type] = handler;
  },
  getAttribute(name) {
    if (name === "data-action") {
      return "open-file";
    }
    return null;
  },
  closest(selector) {
    if (selector === "[data-action]") {
      return this;
    }
    return null;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  on(type, selector, handler) {
    if (typeof handler === "function") {
      delegated[type] = { selector, handler };
      return;
    }

    delegated[type] = { selector: null, handler: selector };
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

if (!delegated.ready || typeof delegated.ready.handler !== "function") {
  throw new Error("expected Sciter ready handler registration");
}

const closeButton = createTarget({ action: "window-close" });
delegated.click.handler.call(closeButton, { target: closeButton }, closeButton);

if (calls.length !== 0) {
  throw new Error("window button click should not xcall: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn recent_files_popup_opens_below_file_name_anchor() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
let popupArgs = null;
const menu = { childElementCount: 2 };
const anchor = {
  popup(menuElement, options) {
    popupArgs = { menuElement, options };
  },
};
const openButton = { addEventListener() {} };

global.Window = { this: { xcall() {} } };
global.document = {
  readyState: "loading",
  addEventListener() {},
  getElementById(id) {
    return id === "recent-files-menu" ? menu : null;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === '[data-current-file]') {
      return anchor;
    }
    return null;
  },
};

eval(scriptSource);

globalThis.__mdlumaTestHooks.showRecentFilesPopup(anchor);

if (!popupArgs) {
  throw new Error("expected popup() to be called");
}
if (popupArgs.menuElement !== menu) {
  throw new Error("popup menu mismatch");
}
if (popupArgs.options.anchorAt !== 1 || popupArgs.options.popupAt !== 7) {
  throw new Error("unexpected popup placement: " + JSON.stringify(popupArgs.options));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn titlebar_file_name_drags_natively_and_shows_recent_files_on_right_click() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");
        let index = assets
            .read_text_asset(UiTextAsset::IndexHtml)
            .expect("read index html");

        let output = run_node_assertions(
            &script,
            r#"
const docListeners = [];
let fileContextMenuHandler = null;
let popupArgs = null;
const calls = [];
const menu = { childElementCount: 1 };
const openButton = { addEventListener() {} };
const currentFile = {
  addEventListener(type, handler) {
    if (type === "contextmenu") {
      fileContextMenuHandler = handler;
    }
  },
  popup(menuElement, options) {
    popupArgs = { menuElement, options };
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type) {
    docListeners.push(type);
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === '[data-current-file]') {
      return currentFile;
    }
    return null;
  },
  getElementById() {
    return menu;
  },
};

eval(scriptSource);

if (scriptSource.includes("Window.this.move")) {
  throw new Error("window move should be delegated to native window-caption role");
}
if (docListeners.includes("mousemove") || docListeners.includes("mouseup")) {
  throw new Error("drag should not use document mouse tracking: " + JSON.stringify(docListeners));
}
if (typeof fileContextMenuHandler !== "function") {
  throw new Error("expected file name contextmenu handler");
}
fileContextMenuHandler({ preventDefault() {} });

if (!popupArgs || popupArgs.menuElement !== menu) {
  throw new Error("expected recent files popup from file name context menu");
}
        "#,
        );

        assert!(index.contains("class=\"titlebar-brand\" role=\"window-caption\""));
        assert!(index.contains("class=\"titlebar-drag-region\" role=\"window-caption\""));
        assert!(index.contains("class=\"file-name\" data-current-file"));
        assert!(!index.contains("class=\"file-name\" role=\"window-caption\""));
        assert!(index.contains("<menu.popup id=\"recent-files-menu\""));
        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_copies_current_markdown_selection_only() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
const writes = [];
let currentSelection = "Line 1\nLine 2";

const openButton = {
  addEventListener() {},
};

let statusText = "Copy failed";

const markdownBodyElement = {
  selection: {
    isCollapsed: false,
    toString() {
      return currentSelection;
    },
  },
};

const copyStatus = {
  get textContent() {
    return statusText;
  },
  set textContent(value) {
    statusText = value;
  },
};

global.Clipboard = {
  writeText(text) {
    writes.push(text);
    return true;
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {
    prevented += 1;
  },
});

currentSelection = "Updated selection";
listeners.keydown({
  ctrlKey: false,
  metaKey: true,
  key: "C",
  preventDefault() {
    prevented += 1;
  },
});

if (JSON.stringify(writes) !== JSON.stringify([
  "Line 1\nLine 2",
  "Updated selection",
])) {
  throw new Error("unexpected clipboard writes: " + JSON.stringify(writes));
}

if (prevented !== 2) {
  throw new Error("copy shortcut should prevent default twice, got " + prevented);
}

if (statusText !== "") {
  throw new Error("successful copy should clear copy status, got " + JSON.stringify(statusText));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_ignores_copy_without_markdown_selection() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
let writes = 0;
let statusText = "unchanged";

const openButton = {
  addEventListener() {},
};

const markdownBodyElement = {
  selection: {
    isCollapsed: true,
    toString() {
      return "";
    },
  },
};

const copyStatus = {
  get textContent() {
    return statusText;
  },
  set textContent(value) {
    statusText = value;
  },
};

global.Clipboard = {
  writeText() {
    writes += 1;
    return true;
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {
    prevented += 1;
  },
});

if (writes !== 0) {
  throw new Error("copy without selection should not write clipboard");
}

if (prevented !== 0) {
  throw new Error("copy without selection should not prevent default");
}

if (statusText !== "unchanged") {
  throw new Error("copy without selection should leave copy status unchanged");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_shows_local_message_when_clipboard_returns_false() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
let writes = 0;
let statusText = "";

const openButton = {
  addEventListener() {},
};

const markdownBodyElement = {
  selection: {
    isCollapsed: false,
    toString() {
      return "Selected text";
    },
  },
};

const copyStatus = {
  get textContent() {
    return statusText;
  },
  set textContent(value) {
    statusText = value;
  },
};

global.Clipboard = {
  writeText() {
    writes += 1;
    return false;
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {
    prevented += 1;
  },
});

if (writes !== 1) {
  throw new Error("clipboard should be attempted once on failure");
}

if (prevented !== 0) {
  throw new Error("failed copy should not prevent default");
}

if (statusText.length === 0) {
  throw new Error("clipboard failure should show a local copy status message");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_shows_local_message_when_clipboard_throws() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
let statusText = "";

const openButton = {
  addEventListener() {},
};

const markdownBodyElement = {
  selection: {
    isCollapsed: false,
    toString() {
      return "Selected text";
    },
  },
};

const copyStatus = {
  get textContent() {
    return statusText;
  },
  set textContent(value) {
    statusText = value;
  },
};

global.Clipboard = {
  writeText() {
    throw new Error("clipboard unavailable");
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {
    prevented += 1;
  },
});

if (prevented !== 0) {
  throw new Error("exceptional copy failure should not prevent default");
}

if (statusText.length === 0) {
  throw new Error("clipboard exception should show a local copy status message");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_starts_each_new_document_runtime_with_empty_copy_status() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
function createRuntime(writeTextImpl) {
  const listeners = {};
  let statusText = "";

  const openButton = {
    addEventListener() {},
  };

  const markdownBodyElement = {
    selection: {
      isCollapsed: false,
      toString() {
        return "Selected text";
      },
    },
  };

  const copyStatus = {
    get textContent() {
      return statusText;
    },
    set textContent(value) {
      statusText = value;
    },
  };

  global.Clipboard = {
    writeText: writeTextImpl,
  };

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  removeEventListener(type, handler) {
    delete listeners[type];
  },
  on(type, selector, handler) {
    if (typeof handler === "function") {
      listeners[type] = (event) => handler(event, event.target || event);
      return true;
    }
    listeners[type] = handler;
    return true;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]" || selector === "[data-markdown-selection-host]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

  eval(scriptSource);

  return {
    keydown(event) {
      listeners.keydown(event);
    },
    copyStatus() {
      return statusText;
    },
  };
}

const firstDocument = createRuntime(() => false);
firstDocument.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {},
});

if (firstDocument.copyStatus().length === 0) {
  throw new Error("first document should expose copy failure state");
}

const secondDocument = createRuntime(() => true);
if (secondDocument.copyStatus() !== "") {
  throw new Error("new document runtime should start with empty copy status");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn copy_interaction_script_stays_viewer_only_without_xcall_or_markdown_mutation() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
const xcalls = [];
const writes = [];
let statusText = "stale";

const openButton = {
  addEventListener() {},
};

const markdownBodyElement = {
  selection: {
    isCollapsed: false,
    toString() {
      return "Viewer only selection";
    },
  },
  set textContent(_value) {
    throw new Error("copy handler must not mutate markdown textContent");
  },
  set innerHTML(_value) {
    throw new Error("copy handler must not mutate markdown innerHTML");
  },
};

const copyStatus = {
  get textContent() {
    return statusText;
  },
  set textContent(value) {
    statusText = value;
  },
};

global.Window = {
  this: {
    xcall(name) {
      xcalls.push(name);
    },
  },
};

global.Clipboard = {
  writeText(text) {
    writes.push(text);
    return true;
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-markdown-body]") {
      return markdownBodyElement;
    }
    if (selector === "[data-copy-status]") {
      return copyStatus;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  key: "c",
  preventDefault() {
    prevented += 1;
  },
});

if (JSON.stringify(writes) !== JSON.stringify(["Viewer only selection"])) {
  throw new Error("copy should write only the selected viewer text: " + JSON.stringify(writes));
}

if (prevented !== 1) {
  throw new Error("successful viewer-only copy should prevent default once, got " + prevented);
}

if (statusText !== "") {
  throw new Error("successful viewer-only copy should only clear copy status");
}

if (xcalls.length !== 0) {
  throw new Error("copy must stay in UI runtime without xcall: " + JSON.stringify(xcalls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn keyboard_shortcut_ctrl_o_requests_open_file_once() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
const calls = [];
const originalProcess = global.process;
let openClicks = 0;

const openButton = {
  addEventListener() {},
  click() {
    openClicks += 1;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  activeElement: null,
  on(type, selector, handler) {
    if (typeof handler === "function") {
      listeners[type] = handler;
      return true;
    }
    listeners[type] = selector;
    return true;
  },
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

global.process = undefined;

eval(scriptSource);

let prevented = 0;
listeners["^keydown"]({
  ctrlKey: true,
  metaKey: false,
  key: "o",
  code: "KeyO",
  preventDefault() {
    prevented += 1;
  },
});

if (calls.length !== 0) {
  throw new Error("ctrl+o should not require xcall when button click is available, got: " + JSON.stringify(calls));
}

if (openClicks !== 1) {
  throw new Error("ctrl+o should click open button once, got " + openClicks);
}

if (prevented !== 1) {
  throw new Error("ctrl+o should prevent default once, got " + prevented);
}

global.process = originalProcess;
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn keyboard_shortcut_ctrl_w_requests_window_close_once() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
const calls = [];
let closeClicks = 0;

const closeButton = {
  click() {
    closeClicks += 1;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  activeElement: null,
  on(type, selector, handler) {
    if (typeof handler === "function") {
      listeners[type] = handler;
      return true;
    }
    listeners[type] = selector;
    return true;
  },
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return { addEventListener() {} };
    }
    if (selector === '[data-action="window-close"]') {
      return closeButton;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: true,
  metaKey: false,
  altKey: false,
  key: "w",
  code: "KeyW",
  preventDefault() {
    prevented += 1;
  },
});

if (calls.length !== 0) {
  throw new Error("ctrl+w should not xcall, got: " + JSON.stringify(calls));
}

if (closeClicks !== 1) {
  throw new Error("ctrl+w should click close button once, got " + closeClicks);
}

if (prevented !== 1) {
  throw new Error("ctrl+w should prevent default once, got " + prevented);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn keyboard_shortcut_alt_f4_requests_window_close_once() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const listeners = {};
const calls = [];
let closeClicks = 0;

const closeButton = {
  click() {
    closeClicks += 1;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  activeElement: null,
  on(type, selector, handler) {
    if (typeof handler === "function") {
      listeners[type] = handler;
      return true;
    }
    listeners[type] = selector;
    return true;
  },
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return { addEventListener() {} };
    }
    if (selector === '[data-action="window-close"]') {
      return closeButton;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  ctrlKey: false,
  metaKey: false,
  altKey: true,
  key: "F4",
  code: "F4",
  keyCode: 115,
  preventDefault() {
    prevented += 1;
  },
});

if (calls.length !== 0) {
  throw new Error("alt+f4 should not xcall, got: " + JSON.stringify(calls));
}

if (closeClicks !== 1) {
  throw new Error("alt+f4 should click close button once, got " + closeClicks);
}

if (prevented !== 1) {
  throw new Error("alt+f4 should prevent default once, got " + prevented);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn theme_toggle_click_sends_xcall_to_rust() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const buttonListeners = {};
const openButton = {
  disabled: false,
  getAttribute(name) {
    if (name === "data-action") {
      return "open-file";
    }
    return null;
  },
  closest(selector) {
    if (selector === "[data-action]") {
      return this;
    }
    return null;
  },
  addEventListener(type, handler) {
    buttonListeners[type] = handler;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const themeButton = createTarget({ action: "theme" });
listeners.click({ target: createTarget({ actionTarget: themeButton }) });
buttonListeners.click({ currentTarget: themeButton });

if (JSON.stringify(calls) !== JSON.stringify([
  "theme-toggle-requested",
])) {
  throw new Error("expected theme-toggle-requested xcall, got: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_theme_action_sends_theme_toggle_requested_xcall() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const openButton = {
  addEventListener() {},
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const themeButton = createTarget({ action: "theme" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: themeButton }));

if (JSON.stringify(calls) !== JSON.stringify([
  "theme-toggle-requested",
])) {
  throw new Error("expected theme-toggle-requested xcall, got: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_external_editor_action_sends_external_editor_requested_xcall() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const openButton = {
  addEventListener() {},
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const externalEditorItem = createTarget({ action: "external-editor" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: externalEditorItem }));

if (JSON.stringify(calls) !== JSON.stringify([
  "external-editor-requested",
])) {
  throw new Error("expected external-editor-requested xcall, got: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_ignores_disabled_theme_button() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const openButton = {
  addEventListener() {},
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const disabledThemeButton = createTarget({ action: "theme", disabled: true });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: disabledThemeButton }));

if (calls.length !== 0) {
  throw new Error("disabled theme button should be ignored: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn font_click_does_not_send_xcall_directly() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const openButton = {
  addEventListener() {},
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    return null;
  },
};

eval(scriptSource);

const fontButton = createTarget({ action: "font" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: fontButton }));

if (calls.length !== 0) {
  throw new Error("font click must not xcall directly, got: " + JSON.stringify(calls));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_about_action_shows_about_overlay() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const aboutOkButton = {
  focusCalls: 0,
  focus() {
    this.focusCalls += 1;
  },
};

const aboutOverlay = {
  hidden: true,
  style: { display: "none" },
  removeAttribute(name) {
    if (name === "hidden") {
      this.hidden = false;
    }
  },
  setAttribute(name, value) {
    if (name === "hidden") {
      this.hidden = true;
    }
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-about-overlay]") {
      return aboutOverlay;
    }
    if (selector === '[data-action="about-ok"]') {
      return aboutOkButton;
    }
    return null;
  },
};

eval(scriptSource);

const aboutButton = createTarget({ action: "about" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: aboutButton }));

if (aboutOverlay.hidden) {
  throw new Error("about overlay should be visible after About click");
}
if (aboutOverlay.style.display !== "flex") {
  throw new Error("about overlay should use flex display, got " + JSON.stringify(aboutOverlay.style.display));
}
if (aboutOkButton.focusCalls !== 1) {
  throw new Error("about ok button should be focused once, got " + aboutOkButton.focusCalls);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_about_ok_action_hides_about_overlay() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const openButton = {
  addEventListener() {},
};

const aboutOverlay = {
  hidden: false,
  style: { display: "flex" },
  removeAttribute(name) {
    if (name === "hidden") {
      this.hidden = false;
    }
  },
  setAttribute(name, value) {
    if (name === "hidden") {
      this.hidden = true;
    }
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-about-overlay]") {
      return aboutOverlay;
    }
    return null;
  },
};

eval(scriptSource);

const aboutOkButton = createTarget({ action: "about-ok" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: aboutOkButton }));

if (!aboutOverlay.hidden) {
  throw new Error("about overlay should be hidden after OK click");
}
if (aboutOverlay.style.display !== "none") {
  throw new Error("about overlay should use none display, got " + JSON.stringify(aboutOverlay.style.display));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn handle_click_error_ok_action_sends_error_dismiss_xcall() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const openButton = { addEventListener() {} };
const errorOkEl = {
  focusCalls: 0,
  addEventListener() {},
  focus() { this.focusCalls += 1; },
};
const errorOverlay = {
  hidden: false,
  style: { display: "flex" },
  removeAttribute(name) {
    if (name === "hidden") {
      this.hidden = false;
    }
  },
  setAttribute(name, value) {
    if (name === "hidden") {
      this.hidden = true;
    }
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-error-overlay]") {
      return errorOverlay;
    }
    if (selector === '[data-action="error-ok"]') {
      return errorOkEl;
    }
    return null;
  },
};

eval(scriptSource);

const errorOkTarget = createTarget({ action: "error-ok" });
globalThis.__mdlumaTestHooks.handleClick(createTarget({ actionTarget: errorOkTarget }));

if (JSON.stringify(calls) !== JSON.stringify([
  "error-dismiss-requested",
])) {
  throw new Error("expected error-dismiss-requested xcall, got: " + JSON.stringify(calls));
}
if (!errorOverlay.hidden) {
  throw new Error("error overlay should be hidden after OK click");
}
if (errorOverlay.style.display !== "none") {
  throw new Error("error overlay should use none display, got " + JSON.stringify(errorOverlay.style.display));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn escape_key_with_error_overlay_sends_error_dismiss_xcall() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const openButton = { addEventListener() {} };
const errorOverlay = {
  hidden: false,
  style: { display: "flex" },
  removeAttribute(name) {
    if (name === "hidden") {
      this.hidden = false;
    }
  },
  setAttribute(name, value) {
    if (name === "hidden") {
      this.hidden = true;
    }
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-error-overlay]") {
      return errorOverlay;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  key: "Escape",
  code: "Escape",
  keyCode: 27,
  preventDefault() { prevented += 1; },
});

if (JSON.stringify(calls) !== JSON.stringify([
  "error-dismiss-requested",
])) {
  throw new Error("expected error-dismiss-requested xcall, got: " + JSON.stringify(calls));
}
if (prevented !== 1) {
  throw new Error("Escape should prevent default once, got " + prevented);
}
if (!errorOverlay.hidden) {
  throw new Error("error overlay should be hidden after Escape");
}
if (errorOverlay.style.display !== "none") {
  throw new Error("error overlay should use none display after Escape, got " + JSON.stringify(errorOverlay.style.display));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn escape_key_ignores_hidden_error_overlay() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const calls = [];
const listeners = {};
const openButton = { addEventListener() {} };
const errorOverlay = {
  hidden: true,
  style: { display: "none" },
  getAttribute(name) {
    if (name === "hidden") {
      return "hidden";
    }
    return null;
  },
};

global.Window = {
  this: {
    xcall(name) {
      calls.push(name);
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener(type, handler) {
    listeners[type] = handler;
  },
  querySelector(selector) {
    if (selector === '[data-action="open-file"]') {
      return openButton;
    }
    if (selector === "[data-error-overlay]") {
      return errorOverlay;
    }
    return null;
  },
};

eval(scriptSource);

let prevented = 0;
listeners.keydown({
  key: "Escape",
  code: "Escape",
  keyCode: 27,
  preventDefault() { prevented += 1; },
});

if (calls.length !== 0) {
  throw new Error("hidden overlay should not request dismiss: " + JSON.stringify(calls));
}
if (prevented !== 0) {
  throw new Error("hidden overlay should not consume Escape, got " + prevented);
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn drop_handler_calls_open_dropped_files_xcall_with_single_file_path() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const xcalls = [];

global.Element = class {
  constructor() {
    this._handlers = {};
  }
  on(name, handler) {
    if (!this._handlers[name]) {
      this._handlers[name] = [];
    }
    this._handlers[name].push(handler);
    const self = this;
    this["on" + name] = function(evt) {
      evt.stopPropagation = evt.stopPropagation || function() {};
      return handler.call(self, evt);
    };
    return this;
  }
  componentDidMount() {}
};

global.setTimeout = function(fn) { fn(); return 0; };

global.Window = {
  this: {
    xcall(name, ...args) {
      xcalls.push({ name, args });
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  on() { return true; },
  querySelector(_selector) { return { addEventListener() {}, classList: { add() {}, remove() {} }, setAttribute() {} }; },
};

eval(scriptSource);

if (!FileDropTarget) {
  throw new Error("FileDropTarget not defined");
}

const target = new FileDropTarget();
target.componentDidMount();

const accepted = target.ondragaccept({
  detail: { dataType: "file", data: "C:\\Users\\test\\notes.md" },
});
if (!accepted) {
  throw new Error("file payload should be accepted");
}

target.ondrop({});

const dropCall = xcalls.find((call) => call.name === "open-dropped-files");
if (!dropCall) {
  throw new Error("expected open-dropped-files call, got " + JSON.stringify(xcalls));
}
if (JSON.stringify(dropCall.args) !== JSON.stringify(["C:\\Users\\test\\notes.md"])) {
  throw new Error("unexpected args: " + JSON.stringify(dropCall.args));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn drop_handler_rejects_non_file_payload_in_willacceptdrop() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
global.Element = class {
  constructor() {
    this._handlers = {};
  }
  on(name, handler) {
    if (!this._handlers[name]) {
      this._handlers[name] = [];
    }
    this._handlers[name].push(handler);
    const self = this;
    this["on" + name] = function(evt) {
      evt.stopPropagation = evt.stopPropagation || function() {};
      return handler.call(self, evt);
    };
    return this;
  }
  componentDidMount() {}
};

global.Window = { this: { xcall() {} } };

global.document = {
  readyState: "loading",
  addEventListener() {},
  on() { return true; },
  querySelector(_selector) { return { addEventListener() {}, classList: { add() {}, remove() {} }, setAttribute() {} }; },
};

eval(scriptSource);

if (!FileDropTarget) {
  throw new Error("FileDropTarget not defined");
}

 const target = new FileDropTarget();
 target.componentDidMount();

 let accepted;

accepted = target.ondragaccept({
  detail: { dataType: "text", data: {} },
});
if (accepted) {
  throw new Error("text payload should be rejected");
}

accepted = target.ondragaccept({
  detail: { dataType: "html", data: {} },
});
if (accepted) {
  throw new Error("html payload should be rejected");
}

accepted = target.ondragaccept({
  detail: { dataType: "json", data: {} },
});
if (accepted) {
  throw new Error("json payload should be rejected");
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn drop_handler_passes_multiple_file_paths_in_order() {
        let assets = EmbeddedUiAssets::default();
        let script = assets
            .read_text_asset(UiTextAsset::AppJs)
            .expect("read app js");

        let output = run_node_assertions(
            &script,
            r#"
const xcalls = [];

global.Element = class {
  constructor() {
    this._handlers = {};
  }
  on(name, handler) {
    if (!this._handlers[name]) {
      this._handlers[name] = [];
    }
    this._handlers[name].push(handler);
    const self = this;
    this["on" + name] = function(evt) {
      evt.stopPropagation = evt.stopPropagation || function() {};
      return handler.call(self, evt);
    };
    return this;
  }
  componentDidMount() {}
};
global.setTimeout = function(fn) { fn(); return 0; };

global.Window = {
  this: {
    xcall(name, ...args) {
      xcalls.push({ name, args });
    },
  },
};

global.document = {
  readyState: "loading",
  addEventListener() {},
  on() { return true; },
  querySelector(_selector) { return { addEventListener() {}, classList: { add() {}, remove() {} }, setAttribute() {} }; },
};

eval(scriptSource);

if (!FileDropTarget) {
  throw new Error("FileDropTarget not defined");
}

 const target = new FileDropTarget();
 target.componentDidMount();

 target.ondragaccept({
  detail: {
    dataType: "file",
    data: ["C:\\a\\first.md", "C:\\b\\second.md", "C:\\c\\third.md"],
  },
});

target.ondrop({});

const dropCall = xcalls.find((call) => call.name === "open-dropped-files");
if (!dropCall) {
  throw new Error("expected open-dropped-files call, got " + JSON.stringify(xcalls));
}
if (JSON.stringify(dropCall.args) !== JSON.stringify([
  "C:\\a\\first.md",
  "C:\\b\\second.md",
  "C:\\c\\third.md",
])) {
  throw new Error("unexpected args: " + JSON.stringify(dropCall.args));
}
        "#,
        );

        assert!(output.status.success(), "{}", output.stderr);
    }

    #[test]
    fn icon_data_urls_embed_svg_content() {
        let assets = EmbeddedUiAssets;

        let app = assets
            .icon_data_url(IconName::App, IconTheme::Light)
            .expect("resolve app icon");
        let open = assets
            .icon_data_url(IconName::Open, IconTheme::Dark)
            .expect("resolve open icon");
        let search = assets
            .icon_data_url(IconName::Search, IconTheme::Light)
            .expect("resolve search icon");
        let more = assets
            .icon_data_url(IconName::More, IconTheme::Dark)
            .expect("resolve more icon");
        let minimize = assets
            .icon_data_url(IconName::WindowMinimize, IconTheme::Light)
            .expect("resolve minimize icon");
        let maximize = assets
            .icon_data_url(IconName::WindowMaximize, IconTheme::Dark)
            .expect("resolve maximize icon");
        let close = assets
            .icon_data_url(IconName::WindowClose, IconTheme::Light)
            .expect("resolve close icon");

        for url in [&app, &open, &search, &more, &minimize, &maximize, &close] {
            assert!(url.starts_with("data:image/svg+xml;charset=utf-8,"));
            assert!(url.contains("<svg"));
            assert!(!url.starts_with("http"));
            assert!(!url.contains("CARGO_MANIFEST_DIR"));
        }

        assert!(minimize.contains("M18 32h28"));
        assert!(maximize.contains("rx=%222%22"));
        assert!(close.contains("M20 20l24 24"));
    }

    struct NodeRunOutput {
        status: std::process::ExitStatus,
        stderr: String,
    }

    const NODE_CREATE_TARGET_HELPER: &str = r#"function createTarget(options) {
  const values = options || {};
  return {
    disabled: !!values.disabled,
    getAttribute(name) {
      if (name === "data-action") {
        return values.action || null;
      }
      if (name === "data-recent-index") {
        return values.recentIndex ?? null;
      }
      return null;
    },
    closest(selector) {
      if (selector === "[data-action]") {
        return values.actionTarget || null;
      }
      if (selector === "[data-drag-region]") {
        return values.dragRegion || null;
      }
      return null;
    },
  };
}
"#;

    fn run_node_assertions(script: &str, assertions: &str) -> NodeRunOutput {
        static NODE_ASSERTION_RUN_ID: AtomicU64 = AtomicU64::new(0);

        let harness = format!(
            r#"const scriptSource = {script:?};
{NODE_CREATE_TARGET_HELPER}
{assertions}
"#
        );

        let temp_dir = std::env::temp_dir().join("mdluma-ui-tests");
        fs::create_dir_all(&temp_dir).expect("create temp dir for node assertions");
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let run_id = NODE_ASSERTION_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let script_path = temp_dir.join(format!(
            "ui-assertions-{}-{nonce}-{run_id}.cjs",
            std::process::id()
        ));
        fs::write(&script_path, harness).expect("write node assertion harness");

        let output = Command::new("node")
            .arg(&script_path)
            .output()
            .expect("node should be available for js asset tests");

        let _ = fs::remove_file(&script_path);

        NodeRunOutput {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }
    }
}

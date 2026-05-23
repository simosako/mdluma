use crate::{RenderedDocument, SourceDocument, ViewerError};
use comrak::plugins::syntect::SyntectAdapterBuilder;
use comrak::{markdown_to_html_with_plugins, options::Plugins, Options};
use std::sync::OnceLock;

pub const DEFAULT_CJK_FRIENDLY_EMPHASIS: bool = true;

/// Syntect theme used for syntax highlighting in code fences.
/// Both the light and dark UI themes use a dark background for code blocks, so
/// a single dark theme is applied consistently across both viewer modes.
const SYNTAX_HIGHLIGHT_THEME: &str = "base16-ocean.dark";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownRenderOptions {
    pub cjk_friendly_emphasis: bool,
}

impl Default for MarkdownRenderOptions {
    fn default() -> Self {
        Self {
            cjk_friendly_emphasis: DEFAULT_CJK_FRIENDLY_EMPHASIS,
        }
    }
}

pub trait MarkdownRenderer {
    fn render(&self, source: &SourceDocument) -> Result<RenderedDocument, ViewerError> {
        self.render_with_options(source, MarkdownRenderOptions::default())
    }

    fn render_with_options(
        &self,
        source: &SourceDocument,
        options: MarkdownRenderOptions,
    ) -> Result<RenderedDocument, ViewerError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ComrakMarkdownRenderer;

pub struct MarkdownOptions;

impl MarkdownOptions {
    pub fn gfm_viewer(render_options: MarkdownRenderOptions) -> Options<'static> {
        let mut options = Options::default();
        options.extension.table = true;
        options.extension.strikethrough = true;
        options.extension.autolink = true;
        options.extension.tasklist = true;
        options.extension.tagfilter = true;
        options.extension.alerts = true;
        options.extension.cjk_friendly_emphasis = render_options.cjk_friendly_emphasis;
        options.render.r#unsafe = true; // required for raw HTML in markdown; sanitized downstream by sanitize_body_html()
        options
    }
}

impl MarkdownRenderer for ComrakMarkdownRenderer {
    fn render_with_options(
        &self,
        source: &SourceDocument,
        options: MarkdownRenderOptions,
    ) -> Result<RenderedDocument, ViewerError> {
        static SYNTECT_ADAPTER: OnceLock<comrak::plugins::syntect::SyntectAdapter> =
            OnceLock::new();
        let adapter = SYNTECT_ADAPTER.get_or_init(|| {
            SyntectAdapterBuilder::new()
                .theme(SYNTAX_HIGHLIGHT_THEME)
                .build()
        });

        let mut plugins = Plugins::default();
        plugins.render.codefence_syntax_highlighter = Some(adapter);

        let mut html_body = markdown_to_html_with_plugins(
            &source.markdown,
            &MarkdownOptions::gfm_viewer(options),
            &plugins,
        );

        // Remove trailing newlines in code blocks that cause visual gaps in Sciter.
        // Without syntax highlighting, comrak emits \n directly before </code></pre>.
        // With syntect highlighting, the trailing newline is inside the last token span.
        html_body = html_body.replace("\n</code></pre>", "</code></pre>");
        html_body = html_body.replace("\n</span></code></pre>", "</span></code></pre>");

        Ok(RenderedDocument {
            path: source.path.clone(),
            file_name: source.file_name.clone(),
            base_dir: source.base_dir.clone(),
            html_body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ComrakMarkdownRenderer, MarkdownOptions, MarkdownRenderOptions, MarkdownRenderer};
    use crate::SourceDocument;
    use std::path::PathBuf;

    #[test]
    fn gfm_viewer_options_enable_only_required_gfm_extensions() {
        let options = MarkdownOptions::gfm_viewer(MarkdownRenderOptions::default());

        assert!(options.extension.table);
        assert!(options.extension.strikethrough);
        assert!(options.extension.autolink);
        assert!(options.extension.tasklist);
        assert!(options.extension.tagfilter);
        assert!(options.extension.alerts);
        assert!(options.extension.cjk_friendly_emphasis);
        assert!(!options.extension.footnotes);
        assert!(!options.extension.description_lists);
    }

    #[test]
    fn gfm_viewer_options_can_disable_cjk_friendly_emphasis() {
        let options = MarkdownOptions::gfm_viewer(MarkdownRenderOptions {
            cjk_friendly_emphasis: false,
        });

        assert!(!options.extension.cjk_friendly_emphasis);
    }

    #[test]
    fn renders_gfm_markdown_as_html_body_fragment() {
        let source = source_document(
            "guide.md",
            "# Guide\n\n| Feature | Status |\n| --- | --- |\n| Table | Works |\n\n~~removed~~\n\nVisit www.example.com.\n\n- [x] Done\n- [ ] Todo\n",
        );

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render GFM markdown");

        assert_eq!(rendered.file_name, "guide.md");
        assert_eq!(rendered.base_dir, PathBuf::from(r"C:\\docs"));
        assert!(rendered.html_body.contains("<h1>Guide</h1>"));
        assert!(rendered.html_body.contains("<table>"));
        assert!(rendered.html_body.contains("<del>removed</del>"));
        assert!(rendered
            .html_body
            .contains("<a href=\"http://www.example.com\">www.example.com</a>"));
        assert!(rendered.html_body.contains("type=\"checkbox\""));
        assert!(!rendered.html_body.contains("<html"));
        assert!(!rendered.html_body.contains("<body"));
    }

    #[test]
    fn tagfilter_escapes_disallowed_raw_html_in_viewer_fragment() {
        let source = source_document("unsafe.md", "before <script>alert(1)</script> after");

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render raw HTML with tagfilter");

        assert!(rendered.html_body.contains("&lt;script"));
        assert!(!rendered.html_body.contains("<script>"));
    }

    #[test]
    fn rendered_document_preserves_source_path_identity() {
        let source = source_document("notes.md", "# Notes");
        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render preserves path");

        assert_eq!(rendered.path, source.path);
        assert_eq!(rendered.path, PathBuf::from(r"C:\\docs\\notes.md"));
        assert_eq!(rendered.file_name, "notes.md");
    }

    #[test]
    fn rendered_document_preserves_base_dir_through_rendering() {
        let source = source_document("index.md", "# Home");
        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render preserves base_dir");

        assert_eq!(rendered.base_dir, source.base_dir);
        assert_eq!(rendered.base_dir, PathBuf::from(r"C:\\docs"));
    }

    #[test]
    fn malformed_markdown_returns_best_effort_output_without_panic() {
        let source = source_document(
            "broken.md",
            "# Broken\n\n[unterminated link\n\n| missing | separator\n\n```rust\nfn main() {",
        );

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("malformed markdown should render best-effort HTML");

        assert_eq!(rendered.file_name, "broken.md");
        assert!(rendered.html_body.contains("Broken"));
        assert!(!rendered.html_body.is_empty());
    }

    #[test]
    fn unsupported_non_gfm_syntax_degrades_to_readable_output() {
        let source = source_document(
            "unsupported.md",
            "Paragraph with a footnote[^1].\n\n[^1]: Footnotes stay readable even when unsupported.",
        );

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("unsupported syntax should still render");

        assert!(rendered.html_body.contains("Paragraph with a footnote"));
        assert!(rendered.html_body.contains("[^1]"));
        assert!(rendered
            .html_body
            .contains("Footnotes stay readable even when unsupported."));
    }

    #[test]
    fn renders_cjk_underscore_emphasis_by_default() {
        let source = source_document("ja.md", "“︁Git”︁__Hub__\n\n简体字 / 新字体。︀_Simplified._");

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render CJK underscore emphasis");

        assert!(
            rendered
                .html_body
                .contains("<p>“︁Git”︁<strong>Hub</strong></p>"),
            "{}",
            rendered.html_body
        );
        assert!(
            rendered
                .html_body
                .contains("<p>简体字 / 新字体。︀<em>Simplified.</em></p>"),
            "{}",
            rendered.html_body
        );
    }

    #[test]
    fn can_disable_cjk_underscore_emphasis_per_render() {
        let source = source_document("ja.md", "“︁Git”︁__Hub__\n\n简体字 / 新字体。︀_Simplified._");

        let rendered = ComrakMarkdownRenderer
            .render_with_options(
                &source,
                MarkdownRenderOptions {
                    cjk_friendly_emphasis: false,
                },
            )
            .expect("render without CJK underscore emphasis");

        assert!(rendered.html_body.contains("“︁Git”︁__Hub__"));
        assert!(rendered
            .html_body
            .contains("简体字 / 新字体。︀_Simplified._"));
    }

    #[test]
    fn fenced_code_block_with_language_produces_syntax_highlighted_spans() {
        let source = source_document("code.md", "```rust\nfn main() {}\n```");

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render fenced code block with language");

        // Syntect highlighting wraps tokens in spans with inline color styles
        assert!(
            rendered.html_body.contains("<span style="),
            "highlighted code should contain styled spans, got: {}",
            rendered.html_body
        );
        assert!(
            rendered.html_body.contains("fn") && rendered.html_body.contains("main"),
            "code content should be preserved"
        );
    }

    #[test]
    fn fenced_code_block_without_language_renders_plain_code() {
        let source = source_document("code.md", "```\nplain text block\n```");

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render fenced code block without language");

        assert!(
            rendered.html_body.contains("plain text block"),
            "plain code content should be preserved"
        );
        assert!(
            rendered.html_body.contains("<pre>") || rendered.html_body.contains("<pre "),
            "code should be wrapped in pre element"
        );
    }

    fn source_document(file_name: &str, markdown: &str) -> SourceDocument {
        SourceDocument {
            path: PathBuf::from(r"C:\\docs").join(file_name),
            file_name: file_name.to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            markdown: markdown.to_string(),
        }
    }

    #[test]
    fn renders_gfm_alert_types_with_alert_classes() {
        let md = "> [!NOTE]\n> Note content.\n\n> [!TIP]\n> Tip content.\n\n> [!IMPORTANT]\n> Important content.\n\n> [!WARNING]\n> Warning content.\n\n> [!CAUTION]\n> Caution content.\n";
        let source = source_document("alerts.md", md);

        let rendered = ComrakMarkdownRenderer
            .render(&source)
            .expect("render GFM alerts");

        assert!(rendered.html_body.contains("markdown-alert-note"));
        assert!(rendered.html_body.contains("markdown-alert-tip"));
        assert!(rendered.html_body.contains("markdown-alert-important"));
        assert!(rendered.html_body.contains("markdown-alert-warning"));
        assert!(rendered.html_body.contains("markdown-alert-caution"));
        assert!(rendered.html_body.contains("markdown-alert-title"));
    }
}

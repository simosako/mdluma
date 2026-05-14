use crate::{RenderedDocument, SourceDocument, ViewerError};
use comrak::{markdown_to_html, Options};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkdownRenderOptions {
    pub cjk_friendly_emphasis: bool,
}

impl Default for MarkdownRenderOptions {
    fn default() -> Self {
        Self {
            cjk_friendly_emphasis: true,
        }
    }
}

pub trait MarkdownRenderer {
    fn render(
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
        options.extension.cjk_friendly_emphasis = render_options.cjk_friendly_emphasis;
        options.render.r#unsafe = true; // required for raw HTML in markdown; sanitized downstream by sanitize_body_html()
        options
    }
}

impl MarkdownRenderer for ComrakMarkdownRenderer {
    fn render(
        &self,
        source: &SourceDocument,
        options: MarkdownRenderOptions,
    ) -> Result<RenderedDocument, ViewerError> {
        let mut html_body = markdown_to_html(&source.markdown, &MarkdownOptions::gfm_viewer(options));

        // Remove trailing newlines in code blocks that cause visual gaps in Sciter
        html_body = html_body.replace("\n</code></pre>", "</code></pre>");

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
    use super::{
        ComrakMarkdownRenderer, MarkdownOptions, MarkdownRenderOptions, MarkdownRenderer,
    };
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
            .render(&source, MarkdownRenderOptions::default())
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
            .render(&source, MarkdownRenderOptions::default())
            .expect("render raw HTML with tagfilter");

        assert!(rendered.html_body.contains("&lt;script"));
        assert!(!rendered.html_body.contains("<script>"));
    }

    #[test]
    fn rendered_document_preserves_source_path_identity() {
        let source = source_document("notes.md", "# Notes");
        let rendered = ComrakMarkdownRenderer
            .render(&source, MarkdownRenderOptions::default())
            .expect("render preserves path");

        assert_eq!(rendered.path, source.path);
        assert_eq!(rendered.path, PathBuf::from(r"C:\\docs\\notes.md"));
        assert_eq!(rendered.file_name, "notes.md");
    }

    #[test]
    fn rendered_document_preserves_base_dir_through_rendering() {
        let source = source_document("index.md", "# Home");
        let rendered = ComrakMarkdownRenderer
            .render(&source, MarkdownRenderOptions::default())
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
            .render(&source, MarkdownRenderOptions::default())
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
            .render(&source, MarkdownRenderOptions::default())
            .expect("unsupported syntax should still render");

        assert!(rendered.html_body.contains("Paragraph with a footnote"));
        assert!(rendered.html_body.contains("[^1]"));
        assert!(rendered
            .html_body
            .contains("Footnotes stay readable even when unsupported."));
    }

    #[test]
    fn renders_cjk_underscore_emphasis_by_default() {
        let source = source_document(
            "ja.md",
            "“︁Git”︁__Hub__\n\n简体字 / 新字体。︀_Simplified._",
        );

        let rendered = ComrakMarkdownRenderer
            .render(&source, MarkdownRenderOptions::default())
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
        let source = source_document(
            "ja.md",
            "“︁Git”︁__Hub__\n\n简体字 / 新字体。︀_Simplified._",
        );

        let rendered = ComrakMarkdownRenderer
            .render(
                &source,
                MarkdownRenderOptions {
                    cjk_friendly_emphasis: false,
                },
            )
            .expect("render without CJK underscore emphasis");

        assert!(rendered
            .html_body
            .contains("“︁Git”︁__Hub__"));
        assert!(rendered
            .html_body
            .contains("简体字 / 新字体。︀_Simplified._"));
    }

    fn source_document(file_name: &str, markdown: &str) -> SourceDocument {
        SourceDocument {
            path: PathBuf::from(r"C:\\docs").join(file_name),
            file_name: file_name.to_string(),
            base_dir: PathBuf::from(r"C:\\docs"),
            markdown: markdown.to_string(),
        }
    }
}

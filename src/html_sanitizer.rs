pub(crate) fn sanitize_body_html(html: &str, base_dir: Option<&std::path::Path>) -> String {
    let mut sanitized = String::with_capacity(html.len());
    let mut cursor = 0;

    while let Some(relative_start) = html[cursor..].find('<') {
        let tag_start = cursor + relative_start;
        sanitized.push_str(&html[cursor..tag_start]);

        match find_tag_end(html, tag_start) {
            Some(tag_end) => {
                let tag = &html[tag_start..=tag_end];
                sanitized.push_str(&sanitize_tag(tag, base_dir));
                cursor = tag_end + 1;
            }
            None => {
                sanitized.push_str(&html[tag_start..]);
                return sanitized;
            }
        }
    }

    sanitized.push_str(&html[cursor..]);
    sanitized
}

pub(crate) fn file_url_from_path(path: &std::path::Path) -> Option<String> {
    #[cfg(windows)]
    let normalized = normalize_windows_path_for_file_url(path)?;
    #[cfg(not(windows))]
    let normalized = path.to_string_lossy().replace('\\', "/");

    if normalized.starts_with("//") {
        return Some(format!("file:{}", encode_file_url_path(&normalized)));
    }

    if normalized.len() >= 2 && normalized.as_bytes()[1] == b':' {
        return Some(format!("file:///{}", encode_file_url_path(&normalized)));
    }

    if normalized.starts_with('/') {
        return Some(format!("file://{}", encode_file_url_path(&normalized)));
    }

    Some(format!("file:///{}", encode_file_url_path(&normalized)))
}

#[cfg(windows)]
fn normalize_windows_path_for_file_url(path: &std::path::Path) -> Option<String> {
    use std::path::{Component, Prefix};

    let mut components = path.components();
    let mut normalized = String::new();

    if let Some(Component::Prefix(prefix_component)) = components.next() {
        match prefix_component.kind() {
            Prefix::VerbatimDisk(drive) | Prefix::Disk(drive) => {
                normalized.push((drive as char).to_ascii_uppercase());
                normalized.push(':');
            }
            Prefix::VerbatimUNC(server, share) | Prefix::UNC(server, share) => {
                normalized.push_str("//");
                normalized.push_str(&server.to_string_lossy());
                normalized.push('/');
                normalized.push_str(&share.to_string_lossy());
            }
            Prefix::Verbatim(value) => {
                normalized.push_str(&value.to_string_lossy());
            }
            Prefix::DeviceNS(value) => {
                normalized.push_str(&value.to_string_lossy());
            }
        }
    } else {
        normalized.push_str(path.to_str()?);
    }

    for component in components {
        match component {
            Component::RootDir => {
                if !normalized.ends_with('/') {
                    normalized.push('/');
                }
            }
            Component::Normal(segment) => {
                if !normalized.is_empty() && !normalized.ends_with('/') {
                    normalized.push('/');
                }
                normalized.push_str(&segment.to_string_lossy());
            }
            Component::CurDir => {
                if !normalized.is_empty() && !normalized.ends_with('/') {
                    normalized.push('/');
                }
                normalized.push('.');
            }
            Component::ParentDir => {
                if !normalized.is_empty() && !normalized.ends_with('/') {
                    normalized.push('/');
                }
                normalized.push_str("..");
            }
            Component::Prefix(_) => {}
        }
    }

    Some(normalized)
}

fn find_tag_end(html: &str, tag_start: usize) -> Option<usize> {
    let bytes = html.as_bytes();
    let mut cursor = tag_start + 1;
    let mut quote = None;

    while cursor < bytes.len() {
        match bytes[cursor] {
            b'"' | b'\'' if quote.is_none() => quote = Some(bytes[cursor]),
            b'"' | b'\'' if quote == Some(bytes[cursor]) => quote = None,
            b'>' if quote.is_none() => return Some(cursor),
            _ => {}
        }

        cursor += 1;
    }

    None
}

fn sanitize_tag(tag: &str, base_dir: Option<&std::path::Path>) -> String {
    if tag.len() <= 2 || !tag.starts_with('<') {
        return tag.to_string();
    }

    let Some((tag_name, is_closing)) = parsed_tag_name(tag) else {
        return tag.to_string();
    };

    if !is_allowed_render_body_tag(tag_name) {
        return escape_html(tag);
    }

    if is_closing {
        return tag.to_string();
    }

    let bytes = tag.as_bytes();
    let mut sanitized = String::with_capacity(tag.len());
    let tag_end = tag.len() - 1;
    let mut saw_checkbox_input_type = false;
    let mut saw_disabled_attribute = false;
    let mut saw_self_closing_slash = false;
    let mut remote_href: Option<&str> = None;

    sanitized.push('<');
    let mut cursor = 1;

    while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
        sanitized.push(bytes[cursor] as char);
        cursor += 1;
    }

    while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'/' {
        sanitized.push(bytes[cursor] as char);
        cursor += 1;
    }

    while cursor < tag_end {
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        if cursor >= tag_end {
            break;
        }

        if bytes[cursor] == b'/' {
            saw_self_closing_slash = true;
            cursor += 1;
            continue;
        }

        let name_start = cursor;
        while cursor < tag_end
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'/' | b'>')
        {
            cursor += 1;
        }

        if name_start == cursor {
            cursor += 1;
            continue;
        }

        let name = &tag[name_start..cursor];
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let has_value = cursor < tag_end && bytes[cursor] == b'=';
        let mut quoted = None;
        let mut value = "";

        if has_value {
            cursor += 1;
            while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor >= tag_end {
                break;
            }

            let quote = bytes[cursor];
            if matches!(quote, b'"' | b'\'') {
                quoted = Some(quote as char);
                cursor += 1;
                let value_start = cursor;
                while cursor < tag_end && bytes[cursor] != quote {
                    cursor += 1;
                }
                value = &tag[value_start..cursor.min(tag_end)];
                if cursor < tag_end {
                    cursor += 1;
                }
            } else {
                let value_start = cursor;
                while cursor < tag_end
                    && !bytes[cursor].is_ascii_whitespace()
                    && !matches!(bytes[cursor], b'>' | b'<')
                {
                    cursor += 1;
                }
                value = &tag[value_start..cursor];
            }
        }

        if !is_allowed_render_body_attribute(tag_name, name) {
            continue;
        }

        let len_before_attr = sanitized.len();
        sanitized.push(' ');
        sanitized.push_str(name);

        if has_value {
            let mut sanitized_value =
                sanitize_attribute_value(value, replacement_for_attribute(name));
            if tag_name.eq_ignore_ascii_case("img") && name.eq_ignore_ascii_case("src") {
                if let Some(base_dir) = base_dir {
                    if let Some(resolved) = resolve_relative_resource_to_file_url(value, base_dir) {
                        sanitized_value = resolved;
                    }
                }
            }
            if tag_name.eq_ignore_ascii_case("span") && name.eq_ignore_ascii_case("style") {
                sanitized_value = filter_safe_highlight_style(&sanitized_value);
                if sanitized_value.is_empty() {
                    sanitized.truncate(len_before_attr);
                    continue;
                }
            }
            if tag_name.eq_ignore_ascii_case("input") && name.eq_ignore_ascii_case("type") {
                if !sanitized_value.trim().eq_ignore_ascii_case("checkbox") {
                    return escape_html(tag);
                }
                saw_checkbox_input_type = true;
            }

            if tag_name.eq_ignore_ascii_case("a")
                && name.eq_ignore_ascii_case("href")
                && is_safe_external_url(value)
            {
                remote_href = Some(value);
            }

            sanitized.push('=');
            if let Some(quote) = quoted {
                sanitized.push(quote);
                sanitized.push_str(&sanitized_value);
                sanitized.push(quote);
            } else {
                sanitized.push_str(&sanitized_value);
            }
        } else if tag_name.eq_ignore_ascii_case("input") && name.eq_ignore_ascii_case("disabled") {
            saw_disabled_attribute = true;
        }
    }

    if tag_name.eq_ignore_ascii_case("input") && !saw_checkbox_input_type {
        return escape_html(tag);
    }

    if tag_name.eq_ignore_ascii_case("input") && !saw_disabled_attribute {
        sanitized.push_str(" disabled");
    }

    if let Some(href) = remote_href {
        sanitized.push_str(" data-href=\"");
        sanitized.push_str(&escape_html(href));
        sanitized.push('"');
    }

    if saw_self_closing_slash {
        sanitized.push('/');
    }

    sanitized.push('>');
    sanitized
}

fn parsed_tag_name(tag: &str) -> Option<(&str, bool)> {
    if tag.len() <= 2 || !tag.starts_with('<') {
        return None;
    }

    let bytes = tag.as_bytes();
    let mut cursor = 1;
    let mut is_closing = false;

    match bytes[cursor] {
        b'/' => {
            is_closing = true;
            cursor += 1;
        }
        b'!' | b'?' => return None,
        _ => {}
    }

    while cursor < tag.len() - 1 && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }

    let name_start = cursor;
    while cursor < tag.len() - 1
        && !bytes[cursor].is_ascii_whitespace()
        && !matches!(bytes[cursor], b'/' | b'>')
    {
        cursor += 1;
    }

    (name_start < cursor).then(|| (&tag[name_start..cursor], is_closing))
}

fn is_allowed_render_body_tag(tag_name: &str) -> bool {
    const ALLOWED: &[&str] = &[
        "a", "abbr", "b", "blockquote", "br", "code", "del", "details", "div", "em", "h1", "h2",
        "h3", "h4", "h5", "h6", "hr", "i", "img", "input", "kbd", "li", "mark", "ol", "p", "pre",
        "s", "small", "span", "strong", "sub", "summary", "sup", "table", "tbody", "td", "tfoot",
        "th", "thead", "tr", "u", "ul",
    ];
    ALLOWED.iter().any(|&allowed| tag_name.eq_ignore_ascii_case(allowed))
}

fn is_allowed_render_body_attribute(tag_name: &str, attribute: &str) -> bool {
    if attribute.eq_ignore_ascii_case("class")
        || attribute.eq_ignore_ascii_case("title")
        || attribute.eq_ignore_ascii_case("lang")
        || attribute.eq_ignore_ascii_case("dir")
    {
        return true;
    }

    match tag_name.to_ascii_lowercase().as_str() {
        "a" => attribute.eq_ignore_ascii_case("href"),
        "details" => attribute.eq_ignore_ascii_case("open"),
        "img" => {
            attribute.eq_ignore_ascii_case("src")
                || attribute.eq_ignore_ascii_case("alt")
                || attribute.eq_ignore_ascii_case("width")
                || attribute.eq_ignore_ascii_case("height")
        }
        "input" => {
            attribute.eq_ignore_ascii_case("type")
                || attribute.eq_ignore_ascii_case("checked")
                || attribute.eq_ignore_ascii_case("disabled")
        }
        "ol" => attribute.eq_ignore_ascii_case("start"),
        "span" => attribute.eq_ignore_ascii_case("style"),
        "td" | "th" => {
            attribute.eq_ignore_ascii_case("colspan")
                || attribute.eq_ignore_ascii_case("rowspan")
                || attribute.eq_ignore_ascii_case("align")
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum AttributePolicy {
    ReplaceIfRemote(&'static str),
}

fn replacement_for_attribute(attribute: &str) -> Option<AttributePolicy> {
    if attribute.eq_ignore_ascii_case("href") {
        Some(AttributePolicy::ReplaceIfRemote("#"))
    } else if attribute.eq_ignore_ascii_case("src") {
        Some(AttributePolicy::ReplaceIfRemote(""))
    } else {
        None
    }
}

fn sanitize_attribute_value(value: &str, policy: Option<AttributePolicy>) -> String {
    match policy {
        Some(AttributePolicy::ReplaceIfRemote(replacement))
            if is_remote_resource_reference(value) =>
        {
            replacement.to_string()
        }
        _ => value.to_string(),
    }
}

fn resolve_relative_resource_to_file_url(
    value: &str,
    base_dir: &std::path::Path,
) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || is_remote_resource_reference(trimmed)
        || looks_like_absolute_path_or_url(trimmed)
    {
        return None;
    }

    let joined = base_dir.join(trimmed);
    let absolute = std::path::absolute(joined).ok()?;
    file_url_from_path(&absolute)
}

fn looks_like_absolute_path_or_url(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with('\\')
        || (value.len() >= 2 && value.as_bytes()[1] == b':')
}

fn is_remote_resource_reference(value: &str) -> bool {
    let normalized: String = value
        .trim()
        .chars()
        .filter(|character| !character.is_ascii_whitespace() && !character.is_ascii_control())
        .collect();
    let lower = normalized.to_ascii_lowercase();

    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("ftp://")
        || lower.starts_with("//")
        || lower.starts_with("data:")
        || lower.starts_with("file:")
        || lower.starts_with("javascript:")
        || lower.starts_with("vbscript:")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
}

fn is_safe_external_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Filters a CSS style attribute value to only the properties used by syntax highlighting.
/// Only `color`, `background-color`, `font-style`, and `font-weight` are retained,
/// blocking any potentially dangerous CSS (e.g. positioning, content injection).
fn filter_safe_highlight_style(style: &str) -> String {
    let mut filtered = String::with_capacity(style.len());
    for decl in style.split(';') {
        let decl = decl.trim();
        if decl.is_empty() {
            continue;
        }
        let Some((property, _value)) = decl.split_once(':') else {
            continue;
        };
        match property.trim().to_ascii_lowercase().as_str() {
            "color" | "background-color" | "font-style" | "font-weight" => {
                if !filtered.is_empty() {
                    filtered.push(';');
                }
                filtered.push_str(decl);
            }
            _ => {}
        }
    }
    filtered
}

fn encode_file_url_path(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        let safe = matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'/'
                | b':'
        );

        if safe {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex_upper((byte >> 4) & 0x0F));
            encoded.push(hex_upper(byte & 0x0F));
        }
    }
    encoded
}

fn hex_upper(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + (nibble - 10)) as char,
        _ => '0',
    }
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
    use super::{filter_safe_highlight_style, sanitize_body_html};

    #[test]
    fn span_style_color_passes_through_sanitizer() {
        // Syntect-generated style values have a trailing semicolon; the filter removes it.
        let html = r#"<span style="color:#b48ead;">fn</span>"#;
        let result = sanitize_body_html(html, None);
        // style is kept, trailing semicolon stripped by filter
        assert_eq!(result, r#"<span style="color:#b48ead">fn</span>"#);
    }

    #[test]
    fn span_style_background_color_passes_through_sanitizer() {
        let html = r#"<span style="background-color:#2b303b;color:#c0c5ce;">text</span>"#;
        let result = sanitize_body_html(html, None);
        assert_eq!(
            result,
            r#"<span style="background-color:#2b303b;color:#c0c5ce">text</span>"#
        );
    }

    #[test]
    fn span_style_with_dangerous_property_is_stripped() {
        let html = r#"<span style="position:fixed;top:0;left:0;">overlay</span>"#;
        let result = sanitize_body_html(html, None);
        assert!(!result.contains("position"), "dangerous CSS should be stripped");
        assert!(!result.contains("fixed"), "dangerous CSS value should be stripped");
    }

    #[test]
    fn span_style_with_all_dangerous_properties_removes_style_attribute() {
        let html = r#"<span style="position:fixed;">overlay</span>"#;
        let result = sanitize_body_html(html, None);
        assert!(!result.contains("style="), "style attribute should be removed when all properties are unsafe");
        assert!(result.contains("<span>"), "span element itself should be kept");
    }

    #[test]
    fn span_style_mixes_safe_and_unsafe_properties_retains_safe_only() {
        let html = r#"<span style="color:#abc;position:fixed;font-style:italic;">text</span>"#;
        let result = sanitize_body_html(html, None);
        assert!(result.contains("color:#abc"), "safe color property should pass");
        assert!(result.contains("font-style:italic"), "safe font-style property should pass");
        assert!(!result.contains("position"), "dangerous position property should be stripped");
    }

    #[test]
    fn filter_safe_highlight_style_allows_color() {
        assert_eq!(filter_safe_highlight_style("color:#c0c5ce"), "color:#c0c5ce");
    }

    #[test]
    fn filter_safe_highlight_style_allows_background_color() {
        assert_eq!(
            filter_safe_highlight_style("background-color:#2b303b"),
            "background-color:#2b303b"
        );
    }

    #[test]
    fn filter_safe_highlight_style_allows_font_style() {
        assert_eq!(
            filter_safe_highlight_style("font-style:italic"),
            "font-style:italic"
        );
    }

    #[test]
    fn filter_safe_highlight_style_allows_font_weight() {
        assert_eq!(
            filter_safe_highlight_style("font-weight:bold"),
            "font-weight:bold"
        );
    }

    #[test]
    fn filter_safe_highlight_style_strips_dangerous_properties() {
        assert_eq!(filter_safe_highlight_style("position:fixed"), "");
        assert_eq!(filter_safe_highlight_style("display:none"), "");
        assert_eq!(filter_safe_highlight_style("content:attr(data-x)"), "");
    }

    #[test]
    fn filter_safe_highlight_style_handles_multiple_declarations() {
        let input = "color:#abc;font-style:italic;position:fixed;font-weight:bold";
        let result = filter_safe_highlight_style(input);
        assert!(result.contains("color:#abc"));
        assert!(result.contains("font-style:italic"));
        assert!(result.contains("font-weight:bold"));
        assert!(!result.contains("position"));
    }

    #[test]
    fn filter_safe_highlight_style_handles_empty_input() {
        assert_eq!(filter_safe_highlight_style(""), "");
    }

    #[test]
    fn filter_safe_highlight_style_ignores_malformed_declarations() {
        // declarations without ':' separator are silently dropped
        assert_eq!(filter_safe_highlight_style("malformed"), "");
        // valid followed by malformed
        assert_eq!(filter_safe_highlight_style("color:red;malformed"), "color:red");
    }
}

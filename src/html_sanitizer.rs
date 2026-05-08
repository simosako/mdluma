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

                if is_style_opening_tag(tag) {
                    match find_case_insensitive(&html[cursor..], "</style>") {
                        Some(relative_close_start) => {
                            let close_start = cursor + relative_close_start;
                            sanitized.push_str(&sanitize_style_block(&html[cursor..close_start]));

                            match find_tag_end(html, close_start) {
                                Some(close_end) => {
                                    sanitized.push_str(&sanitize_tag(
                                        &html[close_start..=close_end],
                                        base_dir,
                                    ));
                                    cursor = close_end + 1;
                                }
                                None => {
                                    sanitized
                                        .push_str(&sanitize_tag(&html[close_start..], base_dir));
                                    return sanitized;
                                }
                            }
                        }
                        None => {
                            sanitized.push_str(&sanitize_style_block(&html[cursor..]));
                            return sanitized;
                        }
                    }
                }
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

fn find_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    haystack
        .to_ascii_lowercase()
        .find(&needle.to_ascii_lowercase())
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
    matches!(
        tag_name.to_ascii_lowercase().as_str(),
        "a" | "abbr"
            | "b"
            | "blockquote"
            | "br"
            | "code"
            | "del"
            | "details"
            | "div"
            | "em"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "hr"
            | "i"
            | "img"
            | "input"
            | "kbd"
            | "li"
            | "mark"
            | "ol"
            | "p"
            | "pre"
            | "s"
            | "small"
            | "span"
            | "strong"
            | "sub"
            | "summary"
            | "sup"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "u"
            | "ul"
    )
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
        "td" | "th" => {
            attribute.eq_ignore_ascii_case("colspan")
                || attribute.eq_ignore_ascii_case("rowspan")
                || attribute.eq_ignore_ascii_case("align")
        }
        _ => false,
    }
}

fn is_style_opening_tag(tag: &str) -> bool {
    if !tag.starts_with('<') || tag.len() <= 2 {
        return false;
    }

    let bytes = tag.as_bytes();
    if matches!(bytes[1], b'/' | b'!' | b'?') {
        return false;
    }

    let mut cursor = 1;
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

    tag[name_start..cursor].eq_ignore_ascii_case("style")
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

fn sanitize_style_block(css: &str) -> String {
    if contains_remote_css_reference(css) {
        String::new()
    } else {
        css.to_string()
    }
}

fn contains_remote_css_reference(css: &str) -> bool {
    contains_remote_css_url(css) || contains_remote_css_import(css)
}

fn contains_remote_css_url(css: &str) -> bool {
    let lower = css.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(relative_index) = lower[search_from..].find("url(") {
        let value_start = search_from + relative_index + 4;
        match css[value_start..].find(')') {
            Some(relative_end) => {
                let value = &css[value_start..value_start + relative_end];
                if is_remote_resource_reference(strip_css_wrapping_quotes(value)) {
                    return true;
                }

                search_from = value_start + relative_end + 1;
            }
            None => return false,
        }
    }

    false
}

fn contains_remote_css_import(css: &str) -> bool {
    let lower = css.to_ascii_lowercase();
    let mut search_from = 0;

    while let Some(relative_index) = lower[search_from..].find("@import") {
        let import_start = search_from + relative_index + "@import".len();
        let remainder = &css[import_start..];
        let trimmed = remainder.trim_start();

        if let Some(rest) = trimmed.strip_prefix("url(") {
            if let Some(end) = rest.find(')') {
                if is_remote_resource_reference(strip_css_wrapping_quotes(&rest[..end])) {
                    return true;
                }
            }
        } else {
            let value_end = trimmed
                .find(|character: char| character.is_ascii_whitespace() || character == ';')
                .unwrap_or(trimmed.len());
            if is_remote_resource_reference(strip_css_wrapping_quotes(&trimmed[..value_end])) {
                return true;
            }
        }

        search_from = import_start;
    }

    false
}

fn strip_css_wrapping_quotes(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|character| matches!(character, '\'' | '"'))
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

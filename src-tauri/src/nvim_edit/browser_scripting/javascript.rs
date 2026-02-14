//! JavaScript code for browser element interaction

use std::sync::LazyLock;

// Load JS files at compile time
const GET_ELEMENT_RECT_JS_SRC: &str = include_str!("js/get_element_rect.js");
#[allow(dead_code)]
const GET_CURSOR_POSITION_JS_SRC: &str = include_str!("js/get_cursor_position.js");
const GET_TEXT_AND_CURSOR_JS_SRC: &str = include_str!("js/get_text_and_cursor.js");
const SET_CURSOR_POSITION_JS_TEMPLATE: &str = include_str!("js/set_cursor_position.js");
const SET_ELEMENT_TEXT_JS_TEMPLATE: &str = include_str!("js/set_element_text.js");

/// Minify JavaScript for AppleScript execution (removes comments and unnecessary whitespace)
fn minify_js(js: &str) -> String {
    let mut result = String::with_capacity(js.len());
    let mut in_string = false;
    let mut string_char = ' ';
    let mut in_single_comment = false;
    let mut in_multi_comment = false;
    let mut prev_char = ' ';
    let chars: Vec<char> = js.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        // Handle string literals (don't modify content inside strings)
        if !in_single_comment && !in_multi_comment {
            if !in_string && (c == '"' || c == '\'') {
                in_string = true;
                string_char = c;
                result.push(c);
                prev_char = c;
                i += 1;
                continue;
            } else if in_string {
                result.push(c);
                if c == string_char && prev_char != '\\' {
                    in_string = false;
                }
                prev_char = c;
                i += 1;
                continue;
            }
        }

        // Handle comments
        if !in_string && !in_multi_comment && c == '/' && next == Some('/') {
            in_single_comment = true;
            i += 2; // skip both /
            continue;
        }
        if !in_string && !in_single_comment && c == '/' && next == Some('*') {
            in_multi_comment = true;
            i += 2; // skip /*
            continue;
        }
        if in_single_comment && c == '\n' {
            in_single_comment = false;
            // Add space to prevent token merging
            if !result.is_empty()
                && !result.ends_with(' ')
                && !result.ends_with('{')
                && !result.ends_with('(')
            {
                result.push(' ');
            }
            i += 1;
            continue;
        }
        if in_multi_comment && c == '*' && next == Some('/') {
            in_multi_comment = false;
            i += 2; // skip */
            continue;
        }
        if in_single_comment || in_multi_comment {
            i += 1;
            continue;
        }

        // Handle whitespace
        if c.is_whitespace() {
            // Find next non-whitespace character
            let mut next_idx = i + 1;
            while next_idx < chars.len() && chars[next_idx].is_whitespace() {
                next_idx += 1;
            }
            let next_char = chars.get(next_idx).copied();

            // Check if space is needed between tokens
            if !result.is_empty() {
                let last = result.chars().last().unwrap();
                let need_space = is_identifier_char(last)
                    && next_char.map(|c| is_identifier_char(c)).unwrap_or(false);
                if need_space {
                    result.push(' ');
                }
            }
            prev_char = ' ';
            i += 1;
            continue;
        }

        result.push(c);
        prev_char = c;
        i += 1;
    }

    result
}

/// Check if character can be part of an identifier (needs space separation)
fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$'
}

// Pre-minified JS constants (computed once at startup)
pub static GET_ELEMENT_RECT_JS: LazyLock<String> =
    LazyLock::new(|| minify_js(GET_ELEMENT_RECT_JS_SRC));
#[allow(dead_code)]
pub static GET_CURSOR_POSITION_JS: LazyLock<String> =
    LazyLock::new(|| minify_js(GET_CURSOR_POSITION_JS_SRC));
pub static GET_TEXT_AND_CURSOR_JS: LazyLock<String> =
    LazyLock::new(|| minify_js(GET_TEXT_AND_CURSOR_JS_SRC));

/// JavaScript to set cursor position (line, column) in focused element
pub fn build_set_cursor_position_js(line: usize, column: usize) -> String {
    let js = SET_CURSOR_POSITION_JS_TEMPLATE
        .replace("{{TARGET_LINE}}", &line.to_string())
        .replace("{{TARGET_COL}}", &column.to_string());
    minify_js(&js)
}

/// JavaScript to set text on the focused element (for live sync in webviews)
/// Returns "ok_*" on success (may include element ID after colon), error message on failure
/// If target_element_id is provided, will target that specific element
pub fn build_set_element_text_js(text: &str, target_element_id: Option<&str>) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let encoded = STANDARD.encode(text.as_bytes());
    let js = SET_ELEMENT_TEXT_JS_TEMPLATE
        .replace("{{BASE64_TEXT}}", &encoded)
        .replace("{{TARGET_ELEMENT_ID}}", target_element_id.unwrap_or(""));
    minify_js(&js)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_minify_preserves_strings() {
        let js = r#"var x = "hello world";"#;
        let minified = minify_js(js);
        assert!(minified.contains("\"hello world\""));
    }

    #[test]
    fn test_minify_removes_comments() {
        let js = r#"var x = 1; // comment
var y = 2;"#;
        let minified = minify_js(js);
        assert!(!minified.contains("comment"));
        assert!(minified.contains("var x"));
        assert!(minified.contains("var y"));
    }

    #[test]
    fn test_minify_removes_multiline_comments() {
        let js = r#"var x = 1; /* multi
line
comment */ var y = 2;"#;
        let minified = minify_js(js);
        assert!(!minified.contains("multi"));
        assert!(minified.contains("var x"));
        assert!(minified.contains("var y"));
    }

    #[test]
    fn test_templates_compile() {
        // Verify templates load and minify without panic
        let _ = &*GET_ELEMENT_RECT_JS;
        let _ = &*GET_CURSOR_POSITION_JS;
        let _ = &*GET_TEXT_AND_CURSOR_JS;
        let _ = build_set_cursor_position_js(0, 0);
        let _ = build_set_element_text_js("test", None);
        let _ = build_set_element_text_js("test", Some("my-element-id"));
    }
}

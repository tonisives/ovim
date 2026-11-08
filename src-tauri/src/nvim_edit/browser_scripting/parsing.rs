//! JSON parsing utilities for browser scripting responses

use super::types::{CursorPosition, TextAndCursor, ViewportFrame};
use serde::Deserialize;

/// Extract a number from a JSON string by key
pub fn extract_json_number(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let remaining = &json[start..];

    // Find the end of the number (comma, }, or end of string)
    let end = remaining.find([',', '}']).unwrap_or(remaining.len());

    let num_str = remaining[..end].trim();
    num_str.parse().ok()
}

/// Parse JSON string into ViewportFrame
pub fn parse_viewport_frame_json(json: &str) -> Option<ViewportFrame> {
    let json = json.trim().trim_matches('"');

    let x = extract_json_number(json, "x")?;
    let y = extract_json_number(json, "y")?;
    let width = extract_json_number(json, "width")?;
    let height = extract_json_number(json, "height")?;
    let viewport_height = extract_json_number(json, "viewportHeight");

    log::info!(
        "Parsed viewport frame: x={}, y={}, w={}, h={}, viewport_h={:?}",
        x,
        y,
        width,
        height,
        viewport_height
    );

    Some(ViewportFrame {
        x,
        y,
        width,
        height,
        viewport_height,
    })
}

/// Parse cursor position JSON response
#[allow(dead_code)]
pub fn parse_cursor_position_json(json: &str) -> Option<CursorPosition> {
    if json.is_empty() || json == "null" || json == "missing value" {
        return None;
    }

    let line = extract_json_number(json, "line")? as usize;
    let column = extract_json_number(json, "column")? as usize;

    Some(CursorPosition { line, column })
}

/// Parse text and cursor JSON response
pub fn parse_text_and_cursor_json(json: &str) -> Option<TextAndCursor> {
    if json.is_empty() || json == "null" || json == "missing value" {
        return None;
    }

    #[derive(Deserialize)]
    struct Response {
        text: String,
        cursor: Option<CursorPositionResponse>,
        #[serde(default)]
        found: bool,
    }

    #[derive(Deserialize)]
    struct CursorPositionResponse {
        line: usize,
        column: usize,
    }

    let response: Response = serde_json::from_str(json).ok()?;
    Some(TextAndCursor {
        text: response.text,
        cursor: response.cursor.map(|cursor| CursorPosition {
            line: cursor.line,
            column: cursor.column,
        }),
        found: response.found,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_viewport_frame_json() {
        let json = r#"{"x":100,"y":200,"width":300,"height":50,"viewportHeight":800}"#;
        let frame = parse_viewport_frame_json(json).unwrap();
        assert_eq!(frame.x, 100.0);
        assert_eq!(frame.y, 200.0);
        assert_eq!(frame.width, 300.0);
        assert_eq!(frame.height, 50.0);
        assert_eq!(frame.viewport_height, Some(800.0));
    }

    #[test]
    fn test_extract_json_number() {
        let json = r#"{"x":123,"y":456}"#;
        assert_eq!(extract_json_number(json, "x"), Some(123.0));
        assert_eq!(extract_json_number(json, "y"), Some(456.0));
        assert_eq!(extract_json_number(json, "z"), None);
    }

    #[test]
    fn parses_empty_focused_editor() {
        let parsed = parse_text_and_cursor_json(
            r#"{"text":"","cursor":{"line":0,"column":0},"found":true}"#,
        )
        .expect("valid editor response");
        assert!(parsed.found);
        assert_eq!(parsed.text, "");
        assert_eq!(parsed.cursor.map(|cursor| cursor.column), Some(0));
    }

    #[test]
    fn parses_escaped_editor_text() {
        let parsed = parse_text_and_cursor_json(
            r#"{"text":"line 1\n\"quoted\"\\tail","cursor":null,"found":true}"#,
        )
        .expect("valid editor response");
        assert_eq!(parsed.text, "line 1\n\"quoted\"\\tail");
    }
}

//! Browser scripting via AppleScript to get focused element positions in web browsers

use super::accessibility::ElementFrame;
use std::process::Command;

/// Supported browser types for AppleScript scripting
#[derive(Debug, Clone, Copy)]
pub enum BrowserType {
    Safari,
    Chrome,
    Brave,
    Arc,
}

impl BrowserType {
    /// Get the application name for AppleScript
    fn app_name(&self) -> &'static str {
        match self {
            BrowserType::Safari => "Safari",
            BrowserType::Chrome => "Google Chrome",
            BrowserType::Brave => "Brave Browser",
            BrowserType::Arc => "Arc",
        }
    }
}

/// Browser bundle ID constants
pub const SAFARI_BUNDLE: &str = "com.apple.Safari";
pub const CHROME_BUNDLE: &str = "com.google.Chrome";
pub const ARC_BUNDLE: &str = "company.thebrowser.Browser";
pub const BRAVE_BUNDLE: &str = "com.brave.Browser";
pub const EDGE_BUNDLE: &str = "com.microsoft.edgemac";

/// Detect if a bundle ID corresponds to a scriptable browser
pub fn detect_browser_type(bundle_id: &str) -> Option<BrowserType> {
    match bundle_id {
        SAFARI_BUNDLE => Some(BrowserType::Safari),
        CHROME_BUNDLE | EDGE_BUNDLE => Some(BrowserType::Chrome),
        BRAVE_BUNDLE => Some(BrowserType::Brave),
        ARC_BUNDLE => Some(BrowserType::Arc),
        _ => None,
    }
}

/// JavaScript to get the focused element's viewport-relative position and viewport height
const GET_ELEMENT_RECT_JS: &str = r#"(function() { function findDeepActiveElement(el) { if (el.shadowRoot && el.shadowRoot.activeElement) { return findDeepActiveElement(el.shadowRoot.activeElement); } return el; } var el = document.activeElement; if (!el || el === document.body || el === document.documentElement) return null; if (el.tagName === 'IFRAME') { try { var iframeDoc = el.contentDocument || el.contentWindow.document; if (iframeDoc && iframeDoc.activeElement && iframeDoc.activeElement !== iframeDoc.body) { var iframeRect = el.getBoundingClientRect(); var innerEl = findDeepActiveElement(iframeDoc.activeElement); var innerRect = innerEl.getBoundingClientRect(); return JSON.stringify({ x: Math.round(iframeRect.left + innerRect.left), y: Math.round(iframeRect.top + innerRect.top), width: Math.round(innerRect.width), height: Math.round(innerRect.height), viewportHeight: window.innerHeight }); } } catch(e) {} } el = findDeepActiveElement(el); var rect = el.getBoundingClientRect(); if (rect.width === 0 && rect.height === 0) return null; return JSON.stringify({ x: Math.round(rect.left), y: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height), viewportHeight: window.innerHeight }); })()"#;

/// JavaScript to get cursor position (line, column) from focused element
/// Returns JSON: {line: 0-based, column: 0-based} or null
/// Note: Simplified version focusing on CodeMirror 6
const GET_CURSOR_POSITION_JS: &str = r#"(function(){var e=document.querySelector(".cm-editor");if(e){var s=window.getSelection();if(s.rangeCount>0){var r=s.getRangeAt(0);var l=e.querySelectorAll(".cm-line");for(var i=0;i<l.length;i++){if(l[i].contains(r.startContainer)){var w=document.createTreeWalker(l[i],NodeFilter.SHOW_TEXT,null,false);var n;var c=0;while(n=w.nextNode()){if(n===r.startContainer){c+=r.startOffset;return JSON.stringify({line:i,column:c});}c+=n.textContent.length;}}}}}return null;})()"#;

/// JavaScript to get BOTH text AND cursor position in one call
/// This avoids cursor position being lost between separate calls
/// Returns JSON: {text: string, cursor: {line, column} | null}
/// Note: Uses String.fromCharCode(10) for newline to avoid AppleScript escaping issues
const GET_TEXT_AND_CURSOR_JS: &str = r#"(function(){var NL=String.fromCharCode(10);var result={text:"",cursor:null};var e=document.querySelector(".cm-editor");if(e){var lines=e.querySelectorAll(".cm-line");var textParts=[];for(var j=0;j<lines.length;j++){textParts.push(lines[j].textContent);}result.text=textParts.join(NL);var s=window.getSelection();if(s.rangeCount>0){var r=s.getRangeAt(0);for(var i=0;i<lines.length;i++){if(lines[i].contains(r.startContainer)){var w=document.createTreeWalker(lines[i],NodeFilter.SHOW_TEXT,null,false);var n;var c=0;while(n=w.nextNode()){if(n===r.startContainer){c+=r.startOffset;result.cursor={line:i,column:c};break;}c+=n.textContent.length;}break;}}}}return JSON.stringify(result);})()"#;

/// JavaScript to set cursor position (line, column) in focused element
/// Minified to avoid issues with newline removal breaking // comments
fn build_set_cursor_position_js(line: usize, column: usize) -> String {
    // Minified JS - handles CM6, Monaco, input/textarea, contenteditable
    format!(
        r#"(function(){{var NL=String.fromCharCode(10);var targetLine={line};var targetCol={col};var cmEditor=document.querySelector(".cm-editor");if(cmEditor){{var lines=cmEditor.querySelectorAll(".cm-line");if(targetLine<lines.length){{var line=lines[targetLine];var range=document.createRange();var sel=window.getSelection();var walker=document.createTreeWalker(line,NodeFilter.SHOW_TEXT,null,false);var node;var offset=0;var targetNode=null;var targetOffset=0;while(node=walker.nextNode()){{var len=node.textContent.length;if(offset+len>=targetCol){{targetNode=node;targetOffset=targetCol-offset;break;}}offset+=len;}}if(targetNode){{range.setStart(targetNode,Math.min(targetOffset,targetNode.textContent.length));range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_cm6";}}range.setStart(line,0);range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_cm6_empty";}}}}if(typeof monaco!=="undefined"&&monaco.editor){{var editors=monaco.editor.getEditors();if(editors&&editors.length>0){{var editor=editors[0];editor.setPosition({{lineNumber:targetLine+1,column:targetCol+1}});editor.focus();return"ok_monaco";}}}}var el=document.activeElement;if(!el)return"no_element";if(el.tagName==="IFRAME"){{try{{var iframeDoc=el.contentDocument||el.contentWindow.document;if(iframeDoc&&iframeDoc.activeElement){{el=iframeDoc.activeElement;}}}}catch(e){{return"iframe_error";}}}}function findDeep(e){{if(e.shadowRoot&&e.shadowRoot.activeElement)return findDeep(e.shadowRoot.activeElement);return e;}}el=findDeep(el);if(el.tagName==="INPUT"||el.tagName==="TEXTAREA"){{var lines=el.value.split(NL);var pos=0;for(var i=0;i<targetLine&&i<lines.length;i++)pos+=lines[i].length+1;pos+=Math.min(targetCol,(lines[targetLine]||"").length);el.setSelectionRange(pos,pos);return"ok_input";}}if(el.isContentEditable){{var text=el.innerText||el.textContent;var lines=text.split(NL);var pos=0;for(var i=0;i<targetLine&&i<lines.length;i++)pos+=lines[i].length+1;pos+=Math.min(targetCol,(lines[targetLine]||"").length);var range=document.createRange();var sel=window.getSelection();var walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT,null,false);var node;var offset=0;while(node=walker.nextNode()){{var len=node.textContent.length;if(offset+len>=pos){{range.setStart(node,pos-offset);range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_ce";}}offset+=len;}}}}return"unsupported";}})()"#,
        line = line,
        col = column
    )
}

/// JavaScript to set text on the focused element (for live sync in webviews)
/// This handles input, textarea, and contenteditable elements
/// Returns "ok" on success, error message on failure
fn build_set_element_text_js(text: &str) -> String {
    // Use base64 encoding to avoid all escaping issues with quotes and special chars
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let encoded = STANDARD.encode(text.as_bytes());

    format!(
        r#"(function() {{
    // Recursively traverse shadow DOM to find the actual focused element
    function findDeepActiveElement(el) {{
        if (el.shadowRoot && el.shadowRoot.activeElement) {{
            return findDeepActiveElement(el.shadowRoot.activeElement);
        }}
        return el;
    }}

    var el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement) return 'no_element';

    // Handle iframe
    if (el.tagName === 'IFRAME') {{
        try {{
            var iframeDoc = el.contentDocument || el.contentWindow.document;
            if (iframeDoc && iframeDoc.activeElement) {{
                el = iframeDoc.activeElement;
            }}
        }} catch(e) {{ return 'iframe_error'; }}
    }}

    // Handle shadow DOM (recursively for nested shadow roots like Reddit uses)
    el = findDeepActiveElement(el);

    // Decode base64-encoded text
    var text = atob('{}');

    // Detect editor type for debugging
    var editorInfo = 'none';
    if (typeof monaco !== 'undefined') editorInfo = 'monaco';
    else if (document.querySelector('.cm-editor')) editorInfo = 'cm6';
    else if (document.querySelector('.CodeMirror')) editorInfo = 'cm5';
    else if (typeof ace !== 'undefined') editorInfo = 'ace';

    // Check for Monaco Editor first (used by boot.dev, VS Code web, etc.)
    // Monaco stores its instance in a global or on DOM elements
    if (typeof monaco !== 'undefined' && monaco.editor) {{
        var editors = monaco.editor.getEditors();
        if (editors && editors.length > 0) {{
            var editor = editors[0];
            var model = editor.getModel();
            if (model) {{
                // Get full range of document
                var fullRange = model.getFullModelRange();
                // Replace entire content
                editor.executeEdits('ovim-live-sync', [{{
                    range: fullRange,
                    text: text,
                    forceMoveMarkers: true
                }}]);
                return 'ok_monaco';
            }}
        }}
    }}

    // Check for CodeMirror 6 (used by some modern editors)
    // CM6 stores view on the DOM element
    var cmView = el.closest('.cm-editor');
    if (cmView && cmView.cmView) {{
        var view = cmView.cmView;
        view.dispatch({{
            changes: {{from: 0, to: view.state.doc.length, insert: text}}
        }});
        return 'ok_cm6';
    }}

    // Check for CodeMirror 5 (legacy but still common)
    if (el.CodeMirror || (el.closest && el.closest('.CodeMirror'))) {{
        var cm = el.CodeMirror || el.closest('.CodeMirror').CodeMirror;
        if (cm) {{
            cm.setValue(text);
            return 'ok_cm5';
        }}
    }}

    // Check for Ace Editor
    if (typeof ace !== 'undefined') {{
        var aceEditors = document.querySelectorAll('.ace_editor');
        if (aceEditors.length > 0) {{
            var aceEditor = ace.edit(aceEditors[0]);
            if (aceEditor) {{
                aceEditor.setValue(text, -1);
                return 'ok_ace';
            }}
        }}
    }}

    // Handle contenteditable (including Lexical, ProseMirror, etc.)
    if (el.isContentEditable) {{
        // Select all content first
        var selection = window.getSelection();
        var range = document.createRange();
        range.selectNodeContents(el);
        selection.removeAllRanges();
        selection.addRange(range);

        // Check for Lexical editor - these only respond to trusted events
        // so synthetic JS events won't work. Return a specific error so we
        // can fall back to clipboard mode.
        var isLexical = el.hasAttribute('data-lexical-editor') || el.__lexicalEditor;
        if (isLexical) {{
            return 'unsupported_lexical';
        }}

        // Try insertFromPaste first - code editors handle paste as literal text
        // without triggering auto-indent or formatting
        var dataTransfer = new DataTransfer();
        dataTransfer.setData('text/plain', text);
        var inputEvent = new InputEvent('beforeinput', {{
            inputType: 'insertFromPaste',
            data: text,
            dataTransfer: dataTransfer,
            bubbles: true,
            cancelable: true
        }});
        var prevText = el.innerText;
        el.dispatchEvent(inputEvent);
        // Verify text actually changed (some editors handle the event but do nothing)
        if (el.innerText !== prevText) return 'ok_paste';

        // Fallback: try insertReplacementText
        inputEvent = new InputEvent('beforeinput', {{
            inputType: 'insertReplacementText',
            data: text,
            bubbles: true,
            cancelable: true
        }});
        el.dispatchEvent(inputEvent);
        if (el.innerText !== prevText) return 'ok_replacement';

        // Fallback: try character-by-character insertText (works for most editors)
        for (var i = 0; i < text.length; i++) {{
            var charEvent = new InputEvent('beforeinput', {{
                inputType: 'insertText',
                data: text[i],
                bubbles: true,
                cancelable: true
            }});
            el.dispatchEvent(charEvent);
        }}
        if (el.innerText !== prevText) return 'ok_inserttext';

        // Last resort: set innerText directly (loses rich formatting but preserves whitespace)
        el.innerText = text;
        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        return 'ok_innertext_' + editorInfo;
    }}

    // Handle input/textarea
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {{
        // For React/Vue controlled inputs, we need to use native setter
        var nativeInputValueSetter = Object.getOwnPropertyDescriptor(
            el.tagName === 'INPUT' ? window.HTMLInputElement.prototype : window.HTMLTextAreaElement.prototype,
            'value'
        ).set;
        nativeInputValueSetter.call(el, text);

        // Dispatch input event to notify frameworks
        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
        return 'ok_textarea_' + editorInfo;
    }}

    return 'unsupported_' + el.tagName + '_' + editorInfo;
}})()"#,
        encoded
    )
}

/// Set text on the focused element in a browser using AppleScript + JavaScript
/// Returns Ok(()) on success, Err with message on failure
pub fn set_browser_element_text(browser_type: BrowserType, text: &str) -> Result<(), String> {
    let js = build_set_element_text_js(text);

    let script = match browser_type {
        BrowserType::Safari => build_safari_execute_script(&js),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_execute_script(browser_type.app_name(), &js)
        }
    };

    // Debug: write script to file for inspection
    let _ = std::fs::write("/tmp/set_text_script.txt", &script);
    log::info!("set_browser_element_text: browser={:?}, text_len={}, script_len={}", browser_type, text.len(), script.len());

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    log::info!("set_browser_element_text: exit_code={}, stdout='{}', stderr='{}'", output.status, stdout, stderr);

    if !output.status.success() {
        return Err(format!("AppleScript failed: {}", stderr));
    }

    if stdout.starts_with("ok") {
        log::info!("Browser text sync succeeded: {}", stdout);
        Ok(())
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Cursor position (0-based line and column)
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

/// Get cursor position from the focused element in a browser
pub fn get_browser_cursor_position(browser_type: BrowserType) -> Option<CursorPosition> {
    let script = match browser_type {
        BrowserType::Safari => build_safari_execute_script(GET_CURSOR_POSITION_JS),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_execute_script(browser_type.app_name(), GET_CURSOR_POSITION_JS)
        }
    };

    // Debug: write script to file for inspection
    let _ = std::fs::write("/tmp/cursor_script.txt", &script);

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        log::info!("get_browser_cursor_position AppleScript failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!("get_browser_cursor_position raw output: '{}'", stdout);

    if stdout.is_empty() || stdout == "null" || stdout == "missing value" {
        return None;
    }

    // Parse JSON: {"line":X,"column":Y}
    let line = extract_json_number(&stdout, "line")? as usize;
    let column = extract_json_number(&stdout, "column")? as usize;

    log::info!("Got browser cursor position: line={}, col={}", line, column);
    Some(CursorPosition { line, column })
}

/// Set cursor position in the focused element in a browser
pub fn set_browser_cursor_position(browser_type: BrowserType, line: usize, column: usize) -> Result<(), String> {
    let js = build_set_cursor_position_js(line, column);

    let script = match browser_type {
        BrowserType::Safari => build_safari_execute_script(&js),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_execute_script(browser_type.app_name(), &js)
        }
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to execute AppleScript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if stdout.starts_with("ok") {
        log::debug!("Set browser cursor position: {}", stdout);
        Ok(())
    } else {
        Err(format!("JavaScript returned: {}", stdout))
    }
}

/// Result of getting text and cursor in one call
pub struct TextAndCursor {
    pub text: String,
    pub cursor: Option<CursorPosition>,
}

/// Get text AND cursor position in a single JS call
/// This is more reliable than separate calls as cursor position won't be lost
pub fn get_browser_text_and_cursor(browser_type: BrowserType) -> Option<TextAndCursor> {
    let script = match browser_type {
        BrowserType::Safari => build_safari_execute_script(GET_TEXT_AND_CURSOR_JS),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_execute_script(browser_type.app_name(), GET_TEXT_AND_CURSOR_JS)
        }
    };

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        log::debug!("get_browser_text_and_cursor AppleScript failed: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!("get_browser_text_and_cursor raw output length: {}", stdout.len());

    if stdout.is_empty() || stdout == "null" || stdout == "missing value" {
        return None;
    }

    // Parse JSON: {"text":"...", "cursor": {"line":X,"column":Y} | null}
    // Simple parsing - find text and cursor fields
    let text_start = stdout.find("\"text\":\"")? + 8;
    let text_end = stdout[text_start..].find("\",\"cursor\"").map(|i| text_start + i)?;
    let text_escaped = &stdout[text_start..text_end];

    // Unescape the text (handle \n, \", etc.)
    let text = text_escaped
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\\\", "\\");

    // Parse cursor if present
    let cursor = if stdout.contains("\"cursor\":null") {
        None
    } else if let Some(cursor_start) = stdout.find("\"cursor\":{") {
        let line = extract_json_number(&stdout[cursor_start..], "line")? as usize;
        let column = extract_json_number(&stdout[cursor_start..], "column")? as usize;
        Some(CursorPosition { line, column })
    } else {
        None
    };

    log::info!("Got text ({} chars) and cursor ({:?}) via JS", text.len(), cursor);
    Some(TextAndCursor { text, cursor })
}

/// Get the focused element frame from a browser using AppleScript
pub fn get_browser_element_frame(browser_type: BrowserType) -> Option<ElementFrame> {
    log::info!(
        "Attempting to get element frame from browser: {:?}",
        browser_type
    );

    // Get window position and size from System Events
    let (window_x, window_y, _window_width, window_height) = get_browser_window_bounds(browser_type.app_name())?;
    log::info!("Browser window bounds: x={}, y={}, h={}", window_x, window_y, window_height);

    // Get element's viewport-relative position via JavaScript
    let script = match browser_type {
        BrowserType::Safari => build_safari_script(),
        BrowserType::Chrome | BrowserType::Brave | BrowserType::Arc => {
            build_chrome_script(browser_type.app_name())
        }
    };

    let frame = execute_applescript_and_parse(&script)?;

    // Calculate chrome height: window height - viewport height
    let chrome_height = window_height - frame.viewport_height.unwrap_or(window_height);
    log::info!("Chrome height: {} (window_h={}, viewport_h={})", chrome_height, window_height, frame.viewport_height.unwrap_or(0.0));

    // Combine: window position + chrome height + viewport-relative element position
    Some(ElementFrame {
        x: window_x + frame.x,
        y: window_y + chrome_height + frame.y,
        width: frame.width,
        height: frame.height,
    })
}

/// Get the browser window's position and size using System Events
fn get_browser_window_bounds(app_name: &str) -> Option<(f64, f64, f64, f64)> {
    let script = format!(
        r#"tell application "System Events"
    set winPos to position of front window of process "{}"
    set winSize to size of front window of process "{}"
    return (item 1 of winPos as text) & "," & (item 2 of winPos as text) & "," & (item 1 of winSize as text) & "," & (item 2 of winSize as text)
end tell"#,
        app_name, app_name
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;

    if !output.status.success() {
        log::warn!("Failed to get window bounds: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.trim().split(',').collect();
    if parts.len() != 4 {
        log::warn!("Unexpected window bounds format: {}", stdout);
        return None;
    }

    let x: f64 = parts[0].parse().ok()?;
    let y: f64 = parts[1].parse().ok()?;
    let w: f64 = parts[2].parse().ok()?;
    let h: f64 = parts[3].parse().ok()?;

    Some((x, y, w, h))
}

/// Build AppleScript for Safari
fn build_safari_script() -> String {
    format!(
        r#"tell application "Safari"
    if (count of windows) = 0 then return "null"
    tell front window
        if (count of tabs) = 0 then return "null"
        try
            return do JavaScript "{}" in current tab
        on error
            return "null"
        end try
    end tell
end tell"#,
        GET_ELEMENT_RECT_JS.replace('"', "\\\"")
    )
}

/// Build AppleScript for Chrome-based browsers (Chrome, Arc, Brave, Edge)
fn build_chrome_script(app_name: &str) -> String {
    format!(
        r#"tell application "{}"
    if (count of windows) = 0 then return "null"
    tell active tab of front window
        try
            return execute javascript "{}"
        on error
            return "null"
        end try
    end tell
end tell"#,
        app_name,
        GET_ELEMENT_RECT_JS.replace('"', "\\\"")
    )
}

/// Build AppleScript to execute arbitrary JavaScript in Safari
fn build_safari_execute_script(js: &str) -> String {
    // Escape quotes for AppleScript string - keep newlines as they work fine
    let js_escaped = js
        .replace('\r', "")
        .replace('"', "\\\"");
    format!(
        r#"tell application "Safari"
    if (count of windows) = 0 then return "no_window"
    tell front window
        if (count of tabs) = 0 then return "no_tab"
        try
            return do JavaScript "{}" in current tab
        on error errMsg
            return "error: " & errMsg
        end try
    end tell
end tell"#,
        js_escaped
    )
}

/// Build AppleScript to execute arbitrary JavaScript in Chrome-based browsers
fn build_chrome_execute_script(app_name: &str, js: &str) -> String {
    // Escape quotes for AppleScript string - keep newlines as they work fine
    let js_escaped = js
        .replace('\r', "")
        .replace('"', "\\\"");
    format!(
        r#"tell application "{}"
    if (count of windows) = 0 then return "no_window"
    tell active tab of front window
        try
            return execute javascript "{}"
        on error errMsg
            return "error: " & errMsg
        end try
    end tell
end tell"#,
        app_name,
        js_escaped
    )
}

/// Viewport-relative frame from browser JavaScript
struct ViewportFrame {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    viewport_height: Option<f64>,
}

/// Execute AppleScript and parse the JSON result into ViewportFrame
fn execute_applescript_and_parse(script: &str) -> Option<ViewportFrame> {
    // Execute with timeout
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            log::warn!("Failed to execute AppleScript: {}", e);
            return None;
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        log::warn!("AppleScript failed: {}", stderr);
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    log::info!("AppleScript returned: {}", stdout);

    // Handle "null" or empty response
    if stdout.is_empty() || stdout == "null" || stdout == "missing value" {
        log::info!("Browser returned no element frame");
        return None;
    }

    // Parse JSON
    parse_viewport_frame_json(&stdout)
}

/// Parse JSON string into ViewportFrame
fn parse_viewport_frame_json(json: &str) -> Option<ViewportFrame> {
    // Simple JSON parsing without serde dependency
    // Expected format: {"x":123,"y":456,"width":789,"height":100,"viewportHeight":800}

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

/// Extract a number from a JSON string by key
fn extract_json_number(json: &str, key: &str) -> Option<f64> {
    let pattern = format!("\"{}\":", key);
    let start = json.find(&pattern)? + pattern.len();
    let remaining = &json[start..];

    // Find the end of the number (comma, }, or end of string)
    let end = remaining
        .find(|c: char| c == ',' || c == '}')
        .unwrap_or(remaining.len());

    let num_str = remaining[..end].trim();
    num_str.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_browser_type() {
        assert!(matches!(
            detect_browser_type("com.apple.Safari"),
            Some(BrowserType::Safari)
        ));
        assert!(matches!(
            detect_browser_type("com.google.Chrome"),
            Some(BrowserType::Chrome)
        ));
        assert!(matches!(
            detect_browser_type("company.thebrowser.Browser"),
            Some(BrowserType::Arc)
        ));
        assert!(matches!(
            detect_browser_type("com.brave.Browser"),
            Some(BrowserType::Brave)
        ));
        assert!(detect_browser_type("org.mozilla.firefox").is_none());
        assert!(detect_browser_type("com.apple.TextEdit").is_none());
    }

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
}

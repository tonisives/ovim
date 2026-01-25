//! JavaScript code for browser element interaction

/// JavaScript to get the focused element's viewport-relative position and viewport height
pub const GET_ELEMENT_RECT_JS: &str = r#"(function() { function findDeepActiveElement(el) { if (el.shadowRoot && el.shadowRoot.activeElement) { return findDeepActiveElement(el.shadowRoot.activeElement); } return el; } var el = document.activeElement; if (!el || el === document.body || el === document.documentElement) return null; if (el.tagName === 'IFRAME') { try { var iframeDoc = el.contentDocument || el.contentWindow.document; if (iframeDoc && iframeDoc.activeElement && iframeDoc.activeElement !== iframeDoc.body) { var iframeRect = el.getBoundingClientRect(); var innerEl = findDeepActiveElement(iframeDoc.activeElement); var innerRect = innerEl.getBoundingClientRect(); return JSON.stringify({ x: Math.round(iframeRect.left + innerRect.left), y: Math.round(iframeRect.top + innerRect.top), width: Math.round(innerRect.width), height: Math.round(innerRect.height), viewportHeight: window.innerHeight }); } } catch(e) {} } el = findDeepActiveElement(el); var rect = el.getBoundingClientRect(); if (rect.width === 0 && rect.height === 0) return null; return JSON.stringify({ x: Math.round(rect.left), y: Math.round(rect.top), width: Math.round(rect.width), height: Math.round(rect.height), viewportHeight: window.innerHeight }); })()"#;

/// JavaScript to get cursor position (line, column) from focused element
/// Returns JSON: {line: 0-based, column: 0-based} or null
/// Note: Simplified version focusing on CodeMirror 6
pub const GET_CURSOR_POSITION_JS: &str = r#"(function(){var e=document.querySelector(".cm-editor");if(e){var s=window.getSelection();if(s.rangeCount>0){var r=s.getRangeAt(0);var l=e.querySelectorAll(".cm-line");for(var i=0;i<l.length;i++){if(l[i].contains(r.startContainer)){var w=document.createTreeWalker(l[i],NodeFilter.SHOW_TEXT,null,false);var n;var c=0;while(n=w.nextNode()){if(n===r.startContainer){c+=r.startOffset;return JSON.stringify({line:i,column:c});}c+=n.textContent.length;}}}}}return null;})()"#;

/// JavaScript to get BOTH text AND cursor position in one call
/// This avoids cursor position being lost between separate calls
/// Returns JSON: {text: string, cursor: {line, column} | null}
/// Note: Uses String.fromCharCode(10) for newline to avoid AppleScript escaping issues
pub const GET_TEXT_AND_CURSOR_JS: &str = r#"(function(){var NL=String.fromCharCode(10);var result={text:"",cursor:null};var e=document.querySelector(".cm-editor");if(e){var lines=e.querySelectorAll(".cm-line");var textParts=[];for(var j=0;j<lines.length;j++){textParts.push(lines[j].textContent);}result.text=textParts.join(NL);var s=window.getSelection();if(s.rangeCount>0){var r=s.getRangeAt(0);for(var i=0;i<lines.length;i++){if(lines[i].contains(r.startContainer)){var w=document.createTreeWalker(lines[i],NodeFilter.SHOW_TEXT,null,false);var n;var c=0;while(n=w.nextNode()){if(n===r.startContainer){c+=r.startOffset;result.cursor={line:i,column:c};break;}c+=n.textContent.length;}break;}}}}return JSON.stringify(result);})()"#;

/// JavaScript to set cursor position (line, column) in focused element
/// Minified to avoid issues with newline removal breaking // comments
pub fn build_set_cursor_position_js(line: usize, column: usize) -> String {
    format!(
        r#"(function(){{var NL=String.fromCharCode(10);var targetLine={line};var targetCol={col};var cmEditor=document.querySelector(".cm-editor");if(cmEditor){{var lines=cmEditor.querySelectorAll(".cm-line");if(targetLine<lines.length){{var line=lines[targetLine];var range=document.createRange();var sel=window.getSelection();var walker=document.createTreeWalker(line,NodeFilter.SHOW_TEXT,null,false);var node;var offset=0;var targetNode=null;var targetOffset=0;while(node=walker.nextNode()){{var len=node.textContent.length;if(offset+len>=targetCol){{targetNode=node;targetOffset=targetCol-offset;break;}}offset+=len;}}if(targetNode){{range.setStart(targetNode,Math.min(targetOffset,targetNode.textContent.length));range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_cm6";}}range.setStart(line,0);range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_cm6_empty";}}}}if(typeof monaco!=="undefined"&&monaco.editor){{var editors=monaco.editor.getEditors();if(editors&&editors.length>0){{var editor=editors[0];editor.setPosition({{lineNumber:targetLine+1,column:targetCol+1}});editor.focus();return"ok_monaco";}}}}var el=document.activeElement;if(!el)return"no_element";if(el.tagName==="IFRAME"){{try{{var iframeDoc=el.contentDocument||el.contentWindow.document;if(iframeDoc&&iframeDoc.activeElement){{el=iframeDoc.activeElement;}}}}catch(e){{return"iframe_error";}}}}function findDeep(e){{if(e.shadowRoot&&e.shadowRoot.activeElement)return findDeep(e.shadowRoot.activeElement);return e;}}el=findDeep(el);if(el.tagName==="INPUT"||el.tagName==="TEXTAREA"){{var lines=el.value.split(NL);var pos=0;for(var i=0;i<targetLine&&i<lines.length;i++)pos+=lines[i].length+1;pos+=Math.min(targetCol,(lines[targetLine]||"").length);el.setSelectionRange(pos,pos);return"ok_input";}}if(el.isContentEditable){{var text=el.innerText||el.textContent;var lines=text.split(NL);var pos=0;for(var i=0;i<targetLine&&i<lines.length;i++)pos+=lines[i].length+1;pos+=Math.min(targetCol,(lines[targetLine]||"").length);var range=document.createRange();var sel=window.getSelection();var walker=document.createTreeWalker(el,NodeFilter.SHOW_TEXT,null,false);var node;var offset=0;while(node=walker.nextNode()){{var len=node.textContent.length;if(offset+len>=pos){{range.setStart(node,pos-offset);range.collapse(true);sel.removeAllRanges();sel.addRange(range);return"ok_ce";}}offset+=len;}}}}return"unsupported";}})())"#,
        line = line,
        col = column
    )
}

/// JavaScript to set text on the focused element (for live sync in webviews)
/// This handles input, textarea, and contenteditable elements
/// Returns "ok" on success, error message on failure
pub fn build_set_element_text_js(text: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
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

    // Check if element IS a Lexical editor or is INSIDE one (check ancestors, not descendants)
    function findLexicalEditor(el) {{
        // Check the element itself
        if (el.hasAttribute && el.hasAttribute('data-lexical-editor')) {{
            return el;
        }}
        // Check ancestors using closest (for regular DOM)
        if (el.closest) {{
            var lexicalEl = el.closest('[data-lexical-editor]');
            if (lexicalEl) return lexicalEl;
        }}
        // For shadow DOM, we need to check the host chain
        var current = el;
        while (current) {{
            if (current.hasAttribute && current.hasAttribute('data-lexical-editor')) {{
                return current;
            }}
            // Move to parent or shadow host
            if (current.parentElement) {{
                current = current.parentElement;
            }} else if (current.getRootNode && current.getRootNode().host) {{
                current = current.getRootNode().host;
            }} else {{
                break;
            }}
        }}
        return null;
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

    // Only look for Lexical editor within the focused element's tree (not the whole DOM)
    // This prevents updating a Lexical editor when user has moved focus to search bar
    var lexicalEl = findLexicalEditor(el);
    if (lexicalEl) {{
        el = lexicalEl;
    }}

    // Decode base64-encoded text
    var text = atob('{}');

    // Detect editor type for debugging
    var editorInfo = 'none';
    if (typeof monaco !== 'undefined') editorInfo = 'monaco';
    else if (document.querySelector('.monaco-editor')) editorInfo = 'monaco_dom';
    else if (document.querySelector('.cm-editor')) editorInfo = 'cm6';
    else if (document.querySelector('.CodeMirror')) editorInfo = 'cm5';
    else if (typeof ace !== 'undefined') editorInfo = 'ace';

    // Check for Monaco Editor via script injection (works with coderpad.io, VS Code web, etc.)
    // AppleScript's execute javascript runs in an isolated world, so we need to inject
    // a script tag that runs in the page context to access global variables like 'editor'
    if (document.querySelector('.monaco-editor')) {{
        var commDiv = document.getElementById('__ovimMonacoComm');
        if (!commDiv) {{
            commDiv = document.createElement('div');
            commDiv.id = '__ovimMonacoComm';
            commDiv.style.display = 'none';
            document.body.appendChild(commDiv);
        }}
        // Pass the text via data attribute
        commDiv.setAttribute('data-text', text);
        commDiv.setAttribute('data-result', '');

        // Inject script that runs in page context (can access window.editor and window.monaco)
        var script = document.createElement('script');
        script.textContent = '(function() {{' +
            'var commDiv = document.getElementById("__ovimMonacoComm");' +
            'if (!commDiv) {{ commDiv.setAttribute("data-result", "no_comm_div"); return; }}' +
            'var textToSet = commDiv.getAttribute("data-text") || "";' +
            'try {{' +
                // Method 1: Global editor variable (coderpad.io)
                'if (typeof editor !== "undefined" && typeof editor.executeEdits === "function" && typeof editor.getModel === "function") {{' +
                    'var model = editor.getModel();' +
                    'if (model) {{' +
                        'var fullRange = model.getFullModelRange();' +
                        'editor.executeEdits("ovim-live-sync", [{{ range: fullRange, text: textToSet, forceMoveMarkers: true }}]);' +
                        'commDiv.setAttribute("data-result", "ok_monaco_global");' +
                        'return;' +
                    '}}' +
                '}}' +
                // Method 2: monaco.editor.getEditors() (boot.dev, standard Monaco)
                'if (typeof monaco !== "undefined" && monaco.editor && monaco.editor.getEditors) {{' +
                    'var editors = monaco.editor.getEditors();' +
                    'if (editors && editors.length > 0) {{' +
                        'var ed = editors[0];' +
                        'var model = ed.getModel();' +
                        'if (model) {{' +
                            'var fullRange = model.getFullModelRange();' +
                            'ed.executeEdits("ovim-live-sync", [{{ range: fullRange, text: textToSet, forceMoveMarkers: true }}]);' +
                            'commDiv.setAttribute("data-result", "ok_monaco");' +
                            'return;' +
                        '}}' +
                    '}}' +
                '}}' +
                'commDiv.setAttribute("data-result", "monaco_not_found");' +
            '}} catch(e) {{' +
                'commDiv.setAttribute("data-result", "monaco_error:" + e.message);' +
            '}}' +
        '}})();';
        (document.head || document.documentElement).appendChild(script);
        script.remove();

        // Read result from DOM
        var monacoResult = commDiv.getAttribute('data-result') || 'script_not_run';
        if (monacoResult.indexOf('ok') === 0) {{
            return monacoResult;
        }}
        // If Monaco injection didn't work, continue to other methods
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

        // Check for Lexical editor - use script injection to access page context
        // AppleScript JS runs in isolated world, but injected <script> runs in page context
        // We use DOM (shared between contexts) to pass data and results
        var isLexical = el.hasAttribute('data-lexical-editor');
        if (isLexical) {{
            // Use a hidden div in the DOM to communicate between isolated and page contexts
            var commDiv = document.getElementById('__ovimComm');
            if (!commDiv) {{
                commDiv = document.createElement('div');
                commDiv.id = '__ovimComm';
                commDiv.style.display = 'none';
                document.body.appendChild(commDiv);
            }}
            // Pass the text to page context via DOM
            commDiv.setAttribute('data-text', text);
            commDiv.setAttribute('data-result', '');

            // Inject script that runs in page context (can access __lexicalEditor)
            // Use parseEditorState + setEditorState since $getRoot etc aren't globally exposed
            var script = document.createElement('script');
            script.id = '__ovimLexicalScript';
            script.textContent = '(function() {{' +
                'var commDiv = document.getElementById("__ovimComm");' +
                'if (!commDiv) {{ return; }}' +
                'var textToSet = commDiv.getAttribute("data-text") || "";' +
                'var el = document.activeElement;' +
                'if (!el) {{ commDiv.setAttribute("data-result", "no_element"); return; }}' +
                'var editor = el.__lexicalEditor;' +
                'if (!editor) {{ commDiv.setAttribute("data-result", "no_editor"); return; }}' +
                'try {{' +
                    // Build Lexical state JSON
                    'var lines = textToSet.split(String.fromCharCode(10));' +
                    'var paragraphs = lines.map(function(line) {{' +
                        'if (line.length === 0) {{' +
                            'return {{' +
                                'children: [],' +
                                'direction: null,' +
                                'format: "",' +
                                'indent: 0,' +
                                'type: "paragraph",' +
                                'version: 1' +
                            '}};' +
                        '}}' +
                        'return {{' +
                            'children: [{{' +
                                'detail: 0,' +
                                'format: 0,' +
                                'mode: "normal",' +
                                'style: "",' +
                                'text: line,' +
                                'type: "text",' +
                                'version: 1' +
                            '}}],' +
                            'direction: null,' +
                            'format: "",' +
                            'indent: 0,' +
                            'type: "paragraph",' +
                            'version: 1' +
                        '}};' +
                    '}});' +
                    'var stateJson = {{' +
                        'root: {{' +
                            'children: paragraphs,' +
                            'direction: null,' +
                            'format: "",' +
                            'indent: 0,' +
                            'type: "root",' +
                            'version: 1' +
                        '}}' +
                    '}};' +
                    'var newState = editor.parseEditorState(JSON.stringify(stateJson));' +
                    'editor.setEditorState(newState);' +
                    'commDiv.setAttribute("data-result", "ok_lexical");' +
                '}} catch(e) {{' +
                    'commDiv.setAttribute("data-result", "lexical_error:" + e.message);' +
                '}}' +
            '}})();';
            (document.head || document.documentElement).appendChild(script);
            script.remove();

            // Read result from DOM (shared between contexts)
            var result = commDiv.getAttribute('data-result') || 'script_not_run';
            if (result.indexOf('ok') === 0) {{
                return result;
            }}
            // If script injection failed, fall through to other methods
            if (result !== 'script_not_run') {{
                return result;
            }}
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

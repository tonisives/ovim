// Set text on the focused element (for live sync in webviews)
// This handles input, textarea, and contenteditable elements
// Template variable: {{BASE64_TEXT}} - base64 encoded text to set
// Returns "ok_*" on success, error message on failure
(function () {
  // === Helper Functions ===

  // Recursively traverse shadow DOM to find the actual focused element
  function findDeepActiveElement(el) {
    if (el.shadowRoot && el.shadowRoot.activeElement) {
      return findDeepActiveElement(el.shadowRoot.activeElement);
    }
    return el;
  }

  // Check if element IS a Lexical editor or is INSIDE one (check ancestors, not descendants)
  function findLexicalEditor(el) {
    // Check the element itself
    if (el.hasAttribute && el.hasAttribute("data-lexical-editor")) {
      return el;
    }
    // Check ancestors using closest (for regular DOM)
    if (el.closest) {
      var lexicalEl = el.closest("[data-lexical-editor]");
      if (lexicalEl) return lexicalEl;
    }
    // For shadow DOM, we need to check the host chain
    var current = el;
    while (current) {
      if (current.hasAttribute && current.hasAttribute("data-lexical-editor")) {
        return current;
      }
      // Move to parent or shadow host
      if (current.parentElement) {
        current = current.parentElement;
      } else if (current.getRootNode && current.getRootNode().host) {
        current = current.getRootNode().host;
      } else {
        break;
      }
    }
    return null;
  }

  // Detect editor type for debugging
  function detectEditorType() {
    if (typeof monaco !== "undefined") return "monaco";
    if (document.querySelector(".monaco-editor")) return "monaco_dom";
    if (document.querySelector(".cm-editor")) return "cm6";
    if (document.querySelector(".CodeMirror")) return "cm5";
    if (typeof ace !== "undefined") return "ace";
    return "none";
  }

  // === Monaco Editor Handler ===
  // Uses script injection to access page context (works with coderpad.io, VS Code web, etc.)
  function tryMonaco(text) {
    if (!document.querySelector(".monaco-editor")) return null;

    var commDiv = document.getElementById("__ovimMonacoComm");
    if (!commDiv) {
      commDiv = document.createElement("div");
      commDiv.id = "__ovimMonacoComm";
      commDiv.style.display = "none";
      document.body.appendChild(commDiv);
    }
    commDiv.setAttribute("data-text", text);
    commDiv.setAttribute("data-result", "");

    // Inject script that runs in page context
    var script = document.createElement("script");
    script.textContent =
      "(function() {" +
      'var commDiv = document.getElementById("__ovimMonacoComm");' +
      "if (!commDiv) { commDiv.setAttribute(\"data-result\", \"no_comm_div\"); return; }" +
      'var textToSet = commDiv.getAttribute("data-text") || "";' +
      "try {" +
      // Method 1: Global editor variable (coderpad.io)
      'if (typeof editor !== "undefined" && typeof editor.executeEdits === "function" && typeof editor.getModel === "function") {' +
      "var model = editor.getModel();" +
      "if (model) {" +
      "var fullRange = model.getFullModelRange();" +
      'editor.executeEdits("ovim-live-sync", [{ range: fullRange, text: textToSet, forceMoveMarkers: true }]);' +
      'commDiv.setAttribute("data-result", "ok_monaco_global");' +
      "return;" +
      "}" +
      "}" +
      // Method 2: monaco.editor.getEditors() (boot.dev, standard Monaco)
      'if (typeof monaco !== "undefined" && monaco.editor && monaco.editor.getEditors) {' +
      "var editors = monaco.editor.getEditors();" +
      "if (editors && editors.length > 0) {" +
      "var ed = editors[0];" +
      "var model = ed.getModel();" +
      "if (model) {" +
      "var fullRange = model.getFullModelRange();" +
      'ed.executeEdits("ovim-live-sync", [{ range: fullRange, text: textToSet, forceMoveMarkers: true }]);' +
      'commDiv.setAttribute("data-result", "ok_monaco");' +
      "return;" +
      "}" +
      "}" +
      "}" +
      'commDiv.setAttribute("data-result", "monaco_not_found");' +
      "} catch(e) {" +
      'commDiv.setAttribute("data-result", "monaco_error:" + e.message);' +
      "}" +
      "})();";
    (document.head || document.documentElement).appendChild(script);
    script.remove();

    var result = commDiv.getAttribute("data-result") || "script_not_run";
    if (result.indexOf("ok") === 0) return result;
    return null;
  }

  // === CodeMirror 6 Handler ===
  function tryCodeMirror6(el, text) {
    var cmView = el.closest(".cm-editor");
    if (cmView && cmView.cmView) {
      var view = cmView.cmView;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
      return "ok_cm6";
    }
    return null;
  }

  // === CodeMirror 5 Handler ===
  function tryCodeMirror5(el, text) {
    if (el.CodeMirror || (el.closest && el.closest(".CodeMirror"))) {
      var cm = el.CodeMirror || el.closest(".CodeMirror").CodeMirror;
      if (cm) {
        cm.setValue(text);
        return "ok_cm5";
      }
    }
    return null;
  }

  // === Ace Editor Handler ===
  function tryAce(text) {
    if (typeof ace !== "undefined") {
      var aceEditors = document.querySelectorAll(".ace_editor");
      if (aceEditors.length > 0) {
        var aceEditor = ace.edit(aceEditors[0]);
        if (aceEditor) {
          aceEditor.setValue(text, -1);
          return "ok_ace";
        }
      }
    }
    return null;
  }

  // === Lexical Editor Handler ===
  // Uses script injection to access page context
  function tryLexical(el, text) {
    if (!el.hasAttribute("data-lexical-editor")) return null;

    var commDiv = document.getElementById("__ovimComm");
    if (!commDiv) {
      commDiv = document.createElement("div");
      commDiv.id = "__ovimComm";
      commDiv.style.display = "none";
      document.body.appendChild(commDiv);
    }
    commDiv.setAttribute("data-text", text);
    commDiv.setAttribute("data-result", "");

    // Inject script that runs in page context
    var script = document.createElement("script");
    script.id = "__ovimLexicalScript";
    script.textContent =
      "(function() {" +
      'var commDiv = document.getElementById("__ovimComm");' +
      "if (!commDiv) { return; }" +
      'var textToSet = commDiv.getAttribute("data-text") || "";' +
      "var el = document.activeElement;" +
      'if (!el) { commDiv.setAttribute("data-result", "no_element"); return; }' +
      "var editor = el.__lexicalEditor;" +
      'if (!editor) { commDiv.setAttribute("data-result", "no_editor"); return; }' +
      "try {" +
      // Build Lexical state JSON
      "var lines = textToSet.split(String.fromCharCode(10));" +
      "var paragraphs = lines.map(function(line) {" +
      "if (line.length === 0) {" +
      "return {" +
      "children: []," +
      "direction: null," +
      'format: "",' +
      "indent: 0," +
      'type: "paragraph",' +
      "version: 1" +
      "};" +
      "}" +
      "return {" +
      "children: [{" +
      "detail: 0," +
      "format: 0," +
      'mode: "normal",' +
      'style: "",' +
      "text: line," +
      'type: "text",' +
      "version: 1" +
      "}]," +
      "direction: null," +
      'format: "",' +
      "indent: 0," +
      'type: "paragraph",' +
      "version: 1" +
      "};" +
      "});" +
      "var stateJson = {" +
      "root: {" +
      "children: paragraphs," +
      "direction: null," +
      'format: "",' +
      "indent: 0," +
      'type: "root",' +
      "version: 1" +
      "}" +
      "};" +
      "var newState = editor.parseEditorState(JSON.stringify(stateJson));" +
      "editor.setEditorState(newState);" +
      'commDiv.setAttribute("data-result", "ok_lexical");' +
      "} catch(e) {" +
      'commDiv.setAttribute("data-result", "lexical_error:" + e.message);' +
      "}" +
      "})();";
    (document.head || document.documentElement).appendChild(script);
    script.remove();

    var result = commDiv.getAttribute("data-result") || "script_not_run";
    if (result.indexOf("ok") === 0) return result;
    if (result !== "script_not_run") return result;
    return null;
  }

  // === ContentEditable Handler ===
  function tryContentEditable(el, text, editorInfo) {
    if (!el.isContentEditable) return null;

    // Select all content first
    var selection = window.getSelection();
    var range = document.createRange();
    range.selectNodeContents(el);
    selection.removeAllRanges();
    selection.addRange(range);

    // Try Lexical first
    var lexicalResult = tryLexical(el, text);
    if (lexicalResult) return lexicalResult;

    var prevText = el.innerText;

    // Try insertFromPaste - code editors handle paste as literal text
    var dataTransfer = new DataTransfer();
    dataTransfer.setData("text/plain", text);
    var inputEvent = new InputEvent("beforeinput", {
      inputType: "insertFromPaste",
      data: text,
      dataTransfer: dataTransfer,
      bubbles: true,
      cancelable: true,
    });
    el.dispatchEvent(inputEvent);
    if (el.innerText !== prevText) return "ok_paste";

    // Try insertReplacementText
    inputEvent = new InputEvent("beforeinput", {
      inputType: "insertReplacementText",
      data: text,
      bubbles: true,
      cancelable: true,
    });
    el.dispatchEvent(inputEvent);
    if (el.innerText !== prevText) return "ok_replacement";

    // Try character-by-character insertText
    for (var i = 0; i < text.length; i++) {
      var charEvent = new InputEvent("beforeinput", {
        inputType: "insertText",
        data: text[i],
        bubbles: true,
        cancelable: true,
      });
      el.dispatchEvent(charEvent);
    }
    if (el.innerText !== prevText) return "ok_inserttext";

    // Last resort: set innerText directly
    el.innerText = text;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return "ok_innertext_" + editorInfo;
  }

  // === Input/Textarea Handler ===
  function tryInputTextarea(el, editorInfo) {
    if (el.tagName !== "INPUT" && el.tagName !== "TEXTAREA") return null;

    // For React/Vue controlled inputs, use native setter
    var nativeInputValueSetter = Object.getOwnPropertyDescriptor(
      el.tagName === "INPUT"
        ? window.HTMLInputElement.prototype
        : window.HTMLTextAreaElement.prototype,
      "value"
    ).set;
    nativeInputValueSetter.call(el, text);

    // Dispatch input event to notify frameworks
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return "ok_textarea_" + editorInfo;
  }

  // === Main Logic ===

  var el = document.activeElement;
  if (!el || el === document.body || el === document.documentElement)
    return "no_element";

  // Handle iframe
  if (el.tagName === "IFRAME") {
    try {
      var iframeDoc = el.contentDocument || el.contentWindow.document;
      if (iframeDoc && iframeDoc.activeElement) {
        el = iframeDoc.activeElement;
      }
    } catch (e) {
      return "iframe_error";
    }
  }

  // Handle shadow DOM
  el = findDeepActiveElement(el);

  // Check for Lexical editor within focused element's tree
  var lexicalEl = findLexicalEditor(el);
  if (lexicalEl) {
    el = lexicalEl;
  }

  // Decode base64-encoded text
  var text = atob("{{BASE64_TEXT}}");
  var editorInfo = detectEditorType();

  // Try each editor type in order
  var result;

  result = tryMonaco(text);
  if (result) return result;

  result = tryCodeMirror6(el, text);
  if (result) return result;

  result = tryCodeMirror5(el, text);
  if (result) return result;

  result = tryAce(text);
  if (result) return result;

  result = tryContentEditable(el, text, editorInfo);
  if (result) return result;

  result = tryInputTextarea(el, editorInfo);
  if (result) return result;

  return "unsupported_" + el.tagName + "_" + editorInfo;
})();

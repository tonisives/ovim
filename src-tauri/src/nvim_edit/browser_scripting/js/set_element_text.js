// Set text on the focused element (for live sync in webviews)
// This handles input, textarea, and contenteditable elements
// Template variables:
//   {{BASE64_TEXT}} - base64 encoded text to set
//   {{TARGET_ELEMENT_ID}} - optional element ID from previous call (for Draft.js)
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

  // Check if element IS a Lexical editor or is INSIDE one
  function findLexicalEditor(el) {
    if (el.hasAttribute && el.hasAttribute("data-lexical-editor")) {
      return el;
    }
    if (el.closest) {
      var lexicalEl = el.closest("[data-lexical-editor]");
      if (lexicalEl) return lexicalEl;
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
    if (document.querySelector(".DraftEditor-root")) return "draftjs";
    return "none";
  }

  // === Monaco Editor Handler ===
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

    var script = document.createElement("script");
    script.textContent =
      '(function() {' +
      'var commDiv = document.getElementById("__ovimMonacoComm");' +
      'if (!commDiv) { commDiv.setAttribute("data-result", "no_comm_div"); return; }' +
      'var textToSet = commDiv.getAttribute("data-text") || "";' +
      'try {' +
      'if (typeof editor !== "undefined" && typeof editor.executeEdits === "function" && typeof editor.getModel === "function") {' +
      'var model = editor.getModel();' +
      'if (model) {' +
      'var fullRange = model.getFullModelRange();' +
      'editor.executeEdits("ovim-live-sync", [{ range: fullRange, text: textToSet, forceMoveMarkers: true }]);' +
      'commDiv.setAttribute("data-result", "ok_monaco_global");' +
      'return;' +
      '}' +
      '}' +
      'if (typeof monaco !== "undefined" && monaco.editor && monaco.editor.getEditors) {' +
      'var editors = monaco.editor.getEditors();' +
      'if (editors && editors.length > 0) {' +
      'var ed = editors[0];' +
      'var model = ed.getModel();' +
      'if (model) {' +
      'var fullRange = model.getFullModelRange();' +
      'ed.executeEdits("ovim-live-sync", [{ range: fullRange, text: textToSet, forceMoveMarkers: true }]);' +
      'commDiv.setAttribute("data-result", "ok_monaco");' +
      'return;' +
      '}' +
      '}' +
      '}' +
      'commDiv.setAttribute("data-result", "monaco_not_found");' +
      '} catch(e) {' +
      'commDiv.setAttribute("data-result", "monaco_error:" + e.message);' +
      '}' +
      '})();';
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

  // === Draft.js Editor Handler (Twitter/X, Facebook, etc.) ===
  function tryDraftJS(el, text) {
    if (!el || !el.closest) return null;
    var draftRoot = el.closest(".DraftEditor-root");
    if (!draftRoot) return null;

    var editable = draftRoot.querySelector('[contenteditable="true"]');
    if (!editable) return null;

    // Assign ID for subsequent lookups
    if (!editable.id) {
      editable.id = "ovim-editor-" + Date.now();
    }

    editable.focus();
    document.execCommand("selectAll", false, null);
    var success = document.execCommand("insertText", false, text);
    if (success) {
      return "ok_draftjs:" + editable.id;
    }
    return null;
  }

  // === Lexical Editor Handler ===
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

    var script = document.createElement("script");
    script.textContent =
      '(function() {' +
      'var commDiv = document.getElementById("__ovimComm");' +
      'if (!commDiv) { return; }' +
      'var textToSet = commDiv.getAttribute("data-text") || "";' +
      'var el = document.activeElement;' +
      'if (!el) { commDiv.setAttribute("data-result", "no_element"); return; }' +
      'var editor = el.__lexicalEditor;' +
      'if (!editor) { commDiv.setAttribute("data-result", "no_editor"); return; }' +
      'try {' +
      'var lines = textToSet.split(String.fromCharCode(10));' +
      'var paragraphs = lines.map(function(line) {' +
      'if (line.length === 0) {' +
      'return { children: [], direction: null, format: "", indent: 0, type: "paragraph", version: 1 };' +
      '}' +
      'return { children: [{ detail: 0, format: 0, mode: "normal", style: "", text: line, type: "text", version: 1 }], direction: null, format: "", indent: 0, type: "paragraph", version: 1 };' +
      '});' +
      'var stateJson = { root: { children: paragraphs, direction: null, format: "", indent: 0, type: "root", version: 1 } };' +
      'var newState = editor.parseEditorState(JSON.stringify(stateJson));' +
      'editor.setEditorState(newState);' +
      'commDiv.setAttribute("data-result", "ok_lexical");' +
      '} catch(e) {' +
      'commDiv.setAttribute("data-result", "lexical_error:" + e.message);' +
      '}' +
      '})();';
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

    var selection = window.getSelection();
    var range = document.createRange();
    range.selectNodeContents(el);
    selection.removeAllRanges();
    selection.addRange(range);

    // Try Lexical first
    var lexicalResult = tryLexical(el, text);
    if (lexicalResult) return lexicalResult;

    var prevText = el.innerText;

    // Try insertFromPaste
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

    // Last resort: set innerText directly
    el.innerText = text;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return "ok_innertext_" + editorInfo;
  }

  // === Input/Textarea Handler ===
  function tryInputTextarea(el, text, editorInfo) {
    if (el.tagName !== "INPUT" && el.tagName !== "TEXTAREA") return null;

    var nativeInputValueSetter = Object.getOwnPropertyDescriptor(
      el.tagName === "INPUT"
        ? window.HTMLInputElement.prototype
        : window.HTMLTextAreaElement.prototype,
      "value"
    ).set;
    nativeInputValueSetter.call(el, text);
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return "ok_textarea_" + editorInfo;
  }

  // === Main Logic ===

  var text = atob("{{BASE64_TEXT}}");

  // If we have a cached element ID, try to use it (Draft.js support)
  var targetId = "{{TARGET_ELEMENT_ID}}";
  if (targetId && targetId !== "" && targetId !== "{{" + "TARGET_ELEMENT_ID}}") {
    var targetEl = document.getElementById(targetId);
    if (targetEl) {
      targetEl.focus();
      document.execCommand("selectAll", false, null);
      if (document.execCommand("insertText", false, text)) {
        return "ok_cached:" + targetId;
      }
    }
    // Draft.js may have recreated the element - find it again
    var draftRoot = document.querySelector(".DraftEditor-root");
    if (draftRoot) {
      var editable = draftRoot.querySelector('[contenteditable="true"]');
      if (editable) {
        editable.id = targetId;
        editable.focus();
        document.execCommand("selectAll", false, null);
        if (document.execCommand("insertText", false, text)) {
          return "ok_draftjs_refound:" + targetId;
        }
      }
    }
  }

  var el = document.activeElement;
  if (!el || el === document.body || el === document.documentElement) {
    return "no_element";
  }

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

  // Check for Lexical editor
  var lexicalEl = findLexicalEditor(el);
  if (lexicalEl) {
    el = lexicalEl;
  }

  var editorInfo = detectEditorType();
  var result;

  result = tryMonaco(text);
  if (result) return result;

  result = tryCodeMirror6(el, text);
  if (result) return result;

  result = tryCodeMirror5(el, text);
  if (result) return result;

  result = tryAce(text);
  if (result) return result;

  result = tryDraftJS(el, text);
  if (result) return result;

  result = tryContentEditable(el, text, editorInfo);
  if (result) return result;

  result = tryInputTextarea(el, text, editorInfo);
  if (result) return result;

  return "unsupported_" + el.tagName + "_" + editorInfo;
})();

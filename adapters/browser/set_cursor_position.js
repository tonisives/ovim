// Set cursor position (line, column) in focused element
// Template variables: {{TARGET_LINE}}, {{TARGET_COL}}
// Returns status string: ok_cm6, ok_monaco, ok_input, ok_ce, unsupported, etc.
(function () {
  var NL = String.fromCharCode(10);
  var targetLine = {{TARGET_LINE}};
  var targetCol = {{TARGET_COL}};

  // Try CodeMirror 6 first
  var cmEditor = document.querySelector(".cm-editor");
  if (cmEditor) {
    var lines = cmEditor.querySelectorAll(".cm-line");
    if (targetLine < lines.length) {
      var line = lines[targetLine];
      var range = document.createRange();
      var sel = window.getSelection();
      var walker = document.createTreeWalker(
        line,
        NodeFilter.SHOW_TEXT,
        null,
        false
      );
      var node;
      var offset = 0;
      var targetNode = null;
      var targetOffset = 0;

      while ((node = walker.nextNode())) {
        var len = node.textContent.length;
        if (offset + len >= targetCol) {
          targetNode = node;
          targetOffset = targetCol - offset;
          break;
        }
        offset += len;
      }

      if (targetNode) {
        range.setStart(
          targetNode,
          Math.min(targetOffset, targetNode.textContent.length)
        );
        range.collapse(true);
        sel.removeAllRanges();
        sel.addRange(range);
        return "ok_cm6";
      }

      // Empty line fallback
      range.setStart(line, 0);
      range.collapse(true);
      sel.removeAllRanges();
      sel.addRange(range);
      return "ok_cm6_empty";
    }
  }

  // Try Monaco Editor
  if (typeof monaco !== "undefined" && monaco.editor) {
    var editors = monaco.editor.getEditors();
    if (editors && editors.length > 0) {
      var editor = editors[0];
      editor.setPosition({ lineNumber: targetLine + 1, column: targetCol + 1 });
      editor.focus();
      return "ok_monaco";
    }
  }

  // Get active element (handle iframe and shadow DOM)
  var el = document.activeElement;
  if (!el) return "no_element";

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

  function findDeep(e) {
    if (e.shadowRoot && e.shadowRoot.activeElement)
      return findDeep(e.shadowRoot.activeElement);
    return e;
  }
  el = findDeep(el);

  // Handle input/textarea
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") {
    var lines = el.value.split(NL);
    var pos = 0;
    for (var i = 0; i < targetLine && i < lines.length; i++)
      pos += lines[i].length + 1;
    pos += Math.min(targetCol, (lines[targetLine] || "").length);
    el.setSelectionRange(pos, pos);
    return "ok_input";
  }

  // Handle contenteditable
  if (el.isContentEditable) {
    var text = el.innerText || el.textContent;
    var lines = text.split(NL);
    var pos = 0;
    for (var i = 0; i < targetLine && i < lines.length; i++)
      pos += lines[i].length + 1;
    pos += Math.min(targetCol, (lines[targetLine] || "").length);

    var range = document.createRange();
    var sel = window.getSelection();
    var walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT, null, false);
    var node;
    var offset = 0;

    while ((node = walker.nextNode())) {
      var len = node.textContent.length;
      if (offset + len >= pos) {
        range.setStart(node, pos - offset);
        range.collapse(true);
        sel.removeAllRanges();
        sel.addRange(range);
        return "ok_ce";
      }
      offset += len;
    }
  }

  return "unsupported";
})();

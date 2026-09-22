// Get the focused editor's text and cursor position in one call.
(function () {
  var result = { text: "", cursor: null, found: false };

  function deepActiveElement(element) {
    if (element && element.shadowRoot && element.shadowRoot.activeElement) {
      return deepActiveElement(element.shadowRoot.activeElement);
    }
    return element;
  }

  function cursorFromPrefix(prefix) {
    var lines = prefix.split(String.fromCharCode(10));
    return { line: lines.length - 1, column: lines[lines.length - 1].length };
  }

  var active = deepActiveElement(document.activeElement);
  if (active && (active.tagName === "INPUT" || active.tagName === "TEXTAREA")) {
    result.found = true;
    result.text = active.value || "";
    if (typeof active.selectionStart === "number") {
      result.cursor = cursorFromPrefix(result.text.slice(0, active.selectionStart));
    }
    return JSON.stringify(result);
  }

  var editable = null;
  if (active && active.isContentEditable) {
    editable = active;
  } else if (active && active.closest) {
    editable = active.closest('[contenteditable="true"]');
  }
  if (editable) {
    result.found = true;
    result.text = editable.innerText || "";
    if ((editable.textContent || "") === "") {
      result.text = "";
    }
    var selection = window.getSelection();
    if (selection && selection.rangeCount > 0) {
      var range = selection.getRangeAt(0);
      if (editable.contains(range.startContainer)) {
        var prefixRange = range.cloneRange();
        prefixRange.selectNodeContents(editable);
        prefixRange.setEnd(range.startContainer, range.startOffset);
        result.cursor = cursorFromPrefix(prefixRange.toString());
      }
    }
    return JSON.stringify(result);
  }

  var codeMirror = document.querySelector(".cm-editor");
  if (codeMirror) {
    result.found = true;
    var lines = codeMirror.querySelectorAll(".cm-line");
    var textParts = [];
    for (var j = 0; j < lines.length; j++) {
      textParts.push(lines[j].textContent);
    }
    result.text = textParts.join(String.fromCharCode(10));
  }

  return JSON.stringify(result);
})();

// Get BOTH text AND cursor position in one call
// This avoids cursor position being lost between separate calls
// Returns JSON: {text: string, cursor: {line, column} | null}
// Note: Uses String.fromCharCode(10) for newline to avoid AppleScript escaping issues
(function () {
  var NL = String.fromCharCode(10);
  var result = { text: "", cursor: null };

  var e = document.querySelector(".cm-editor");
  if (e) {
    // Get text from all lines
    var lines = e.querySelectorAll(".cm-line");
    var textParts = [];
    for (var j = 0; j < lines.length; j++) {
      textParts.push(lines[j].textContent);
    }
    result.text = textParts.join(NL);

    // Get cursor position
    var s = window.getSelection();
    if (s.rangeCount > 0) {
      var r = s.getRangeAt(0);
      for (var i = 0; i < lines.length; i++) {
        if (lines[i].contains(r.startContainer)) {
          var w = document.createTreeWalker(
            lines[i],
            NodeFilter.SHOW_TEXT,
            null,
            false
          );
          var n;
          var c = 0;
          while ((n = w.nextNode())) {
            if (n === r.startContainer) {
              c += r.startOffset;
              result.cursor = { line: i, column: c };
              break;
            }
            c += n.textContent.length;
          }
          break;
        }
      }
    }
  }

  return JSON.stringify(result);
})();

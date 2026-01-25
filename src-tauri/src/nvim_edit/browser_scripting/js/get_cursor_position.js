// Get cursor position (line, column) from focused element
// Returns JSON: {line: 0-based, column: 0-based} or null
// Note: Simplified version focusing on CodeMirror 6
(function () {
  var e = document.querySelector(".cm-editor");
  if (e) {
    var s = window.getSelection();
    if (s.rangeCount > 0) {
      var r = s.getRangeAt(0);
      var l = e.querySelectorAll(".cm-line");
      for (var i = 0; i < l.length; i++) {
        if (l[i].contains(r.startContainer)) {
          var w = document.createTreeWalker(
            l[i],
            NodeFilter.SHOW_TEXT,
            null,
            false
          );
          var n;
          var c = 0;
          while ((n = w.nextNode())) {
            if (n === r.startContainer) {
              c += r.startOffset;
              return JSON.stringify({ line: i, column: c });
            }
            c += n.textContent.length;
          }
        }
      }
    }
  }
  return null;
})();

(function () {
  function deepActiveElement(element) {
    if (element && element.shadowRoot && element.shadowRoot.activeElement) {
      return deepActiveElement(element.shadowRoot.activeElement);
    }
    return element;
  }

  // Reddit chat nests its composer inside several open shadow roots.
  function findTarget(root, selector) {
    var found = root.querySelector(selector);
    if (found) return found;
    var elements = root.querySelectorAll("*");
    for (var i = 0; i < elements.length; i++) {
      if (elements[i].shadowRoot) {
        found = findTarget(elements[i].shadowRoot, selector);
        if (found) return found;
      }
    }
    return null;
  }

  var token = "{{TARGET_ELEMENT_ID}}";
  var host = token
    ? findTarget(document, '[data-ovim-editor="' + token + '"]')
    : null;
  var editable = null;

  if (host) {
    editable = host.matches('input, textarea, [contenteditable="true"]')
      ? host
      : host.querySelector('[contenteditable="true"]');
  } else {
    editable = deepActiveElement(document.activeElement);
    if (!editable || editable === document.body || editable === document.documentElement) {
      return "no_element";
    }
    if (!editable.matches('input, textarea, [contenteditable="true"]')) {
      editable = editable.closest('[contenteditable="true"]');
    }
    if (!editable) return "unsupported_element";

    host = editable.closest(".DraftEditor-root") || editable;
    token = "ovim-" + Date.now() + "-" + Math.random().toString(36).slice(2);
    host.setAttribute("data-ovim-editor", token);
  }

  if (!editable) return "target_missing";
  var text = {{NEW_TEXT}};
  var current = editable.tagName === "INPUT" || editable.tagName === "TEXTAREA"
    ? editable.value : editable.innerText;
  if (current === "\n" && !editable.textContent) current = "";
  // Typing at the end needs no selection. Selecting the entire draft on every
  // keystroke also triggers selection-driven formatting popovers on X.
  var append = text.indexOf(current) === 0;
  var kind = host.matches(".DraftEditor-root") ? "draftjs" : "editable";
  var nativeInput = editable.tagName === "INPUT" || editable.tagName === "TEXTAREA";
  var prepared = JSON.stringify({ kind: kind, token: token, append: append,
    nativeInput: nativeInput, text: append ? text.slice(current.length) : text });
  // Initial buffer notifications and duplicate snapshots need no input or focus changes.
  if (text === current) return prepared;

  var cleanups = window.__ovimInputCleanups || (window.__ovimInputCleanups = {});
  var cleanup = cleanups[token];
  if (cleanup && cleanup.editor !== editable) {
    cleanup();
    cleanup = null;
  }
  if (!cleanup) {
    // Keep suppression across updates so focus styling and the browser caret
    // do not flash between CLI calls. Restore it when the edit session ends.
    var style = document.createElement("style");
    style.textContent = '[data-ovim-editor="' + token + '"]::selection, ' +
      '[data-ovim-editor="' + token + '"] ::selection {' +
      'background: transparent !important; color: inherit !important; }' +
      '[data-ovim-editor="' + token + '"], [data-ovim-editor="' + token + '"] * {' +
      'caret-color: transparent !important; }';
    var root = editable.getRootNode();
    (root === document ? (document.head || document.documentElement) : root).appendChild(style);
    var timer;
    var cleaned = false;
    var returnToBrowser = function () { cleanup(); };
    cleanup = function () {
      if (cleaned) return;
      cleaned = true;
      clearTimeout(timer);
      document.removeEventListener("pointerdown", returnToBrowser, true);
      try {
        if (nativeInput) {
          editable.setSelectionRange(editable.selectionEnd, editable.selectionEnd);
        } else {
          var selection = root.getSelection ? root.getSelection() : window.getSelection();
          if (selection && selection.rangeCount && editable.contains(selection.anchorNode) &&
              editable.contains(selection.focusNode)) selection.collapseToEnd();
        }
      } finally {
        // Blurring here triggers focus borders, placeholders and editor toolbars
        // on every subsequent update. Leave DOM focus alone.
        style.remove();
        delete host.__ovimInputCleanup;
        delete cleanups[token];
      }
    };
    cleanup.editor = editable;
    cleanup.refresh = function () {
      clearTimeout(timer);
      // Recover normal styling if Ovim exits without completing the session.
      timer = setTimeout(cleanup, 5000);
    };
    host.__ovimInputCleanup = cleanup;
    cleanups[token] = cleanup;
    document.addEventListener("pointerdown", returnToBrowser, true);
  }
  cleanup.refresh();
  if (deepActiveElement(document.activeElement) !== editable) editable.focus({ preventScroll: true });
  if (editable.tagName === "INPUT" || editable.tagName === "TEXTAREA") {
    editable.setSelectionRange(append ? editable.value.length : 0, editable.value.length);
  }
  // Contenteditable selection belongs to the controlled editor's state.
  // The caller uses trusted Chromium commands instead of DOM ranges.

  return prepared;
})();

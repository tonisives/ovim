(function () {
  function deepActiveElement(element) {
    if (element && element.shadowRoot && element.shadowRoot.activeElement) {
      return deepActiveElement(element.shadowRoot.activeElement);
    }
    return element;
  }

  var token = "{{TARGET_ELEMENT_ID}}";
  var host = token
    ? document.querySelector('[data-ovim-editor="' + token + '"]')
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
  editable.focus();
  if (editable.tagName === "INPUT" || editable.tagName === "TEXTAREA") {
    editable.setSelectionRange(0, editable.value.length);
  } else {
    var selection = window.getSelection();
    var range = document.createRange();
    range.selectNodeContents(editable);
    selection.removeAllRanges();
    selection.addRange(range);
  }

  var kind = host.matches(".DraftEditor-root") ? "draftjs" : "editable";
  return "ok_" + kind + ":" + token;
})();

// Get focused element's viewport-relative position and viewport height
// Returns JSON: {x, y, width, height, viewportHeight} or null
(function () {
  function findDeepActiveElement(el) {
    if (el.shadowRoot && el.shadowRoot.activeElement) {
      return findDeepActiveElement(el.shadowRoot.activeElement);
    }
    return el;
  }

  var el = document.activeElement;
  if (!el || el === document.body || el === document.documentElement) return null;

  // Handle iframe - get element inside iframe
  if (el.tagName === "IFRAME") {
    try {
      var iframeDoc = el.contentDocument || el.contentWindow.document;
      if (
        iframeDoc &&
        iframeDoc.activeElement &&
        iframeDoc.activeElement !== iframeDoc.body
      ) {
        var iframeRect = el.getBoundingClientRect();
        var innerEl = findDeepActiveElement(iframeDoc.activeElement);
        var innerRect = innerEl.getBoundingClientRect();
        return JSON.stringify({
          x: Math.round(iframeRect.left + innerRect.left),
          y: Math.round(iframeRect.top + innerRect.top),
          width: Math.round(innerRect.width),
          height: Math.round(innerRect.height),
          viewportHeight: window.innerHeight,
        });
      }
    } catch (e) {
      // Cross-origin iframe, can't access
    }
  }

  el = findDeepActiveElement(el);
  var rect = el.getBoundingClientRect();

  if (rect.width === 0 && rect.height === 0) return null;

  return JSON.stringify({
    x: Math.round(rect.left),
    y: Math.round(rect.top),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
    viewportHeight: window.innerHeight,
  });
})();

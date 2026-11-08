import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import test from 'node:test';

const template = readFileSync(new URL('../adapters/bmux/prepare_input.js', import.meta.url), 'utf8').replace('{{TARGET_ELEMENT_ID}}', 'test-editor');

function prepare(tagName = 'DIV', shadowDepth = 0, text = 'new') {
  let timeout;
  let removed = false;
  let collapsed = false;
  let style;
  let blurred = false;
  let collapsedRange = false;
  let document;
  const listeners = new Map();
  let focuses = 0;
  let styles = 0;
  const root = { appendChild: value => { style = value; } };
  const editable = {
    tagName, value: "text", innerText: "text", selectionEnd: 4,
    getRootNode: () => shadowDepth ? root : document,
    focus() { focuses++; document.activeElement = editable; },
    blur() { blurred = true; document.activeElement = {}; }, contains: node => node === editable,
    setSelectionRange: (start, end) => { collapsed = start === 4 && end === 4; },
  };
  const selection = { rangeCount: 1, anchorNode: editable, focusNode: editable,
    removeAllRanges() {}, addRange() {}, collapseToEnd: () => { collapsed = true; } };
  const host = { matches: selector => selector === '.DraftEditor-root', querySelector: () => editable };
  let lookupRoot = { querySelector: () => host, querySelectorAll: () => [] };
  for (let i = 0; i < shadowDepth; i++) {
    const child = lookupRoot;
    lookupRoot = { querySelector: () => null, querySelectorAll: () => [{ shadowRoot: child }] };
  }
  document = { activeElement: {}, createRange: () => ({ selectNodeContents() {}, collapse: () => { collapsedRange = true; } }), ...lookupRoot, createElement: () => { styles++; return ({ remove: () => { removed = true; } }); },
      head: { appendChild: value => { style = value; } },
    addEventListener: (name, fn) => listeners.set(name, fn),
    removeEventListener: name => listeners.delete(name) };
  const window = { getSelection: () => selection };
  const context = {
    document, window,
    setTimeout: callback => { timeout = callback; return 1; }, clearTimeout() {},
  };
  const run = text => JSON.parse(runInNewContext(template.replace('{{NEW_TEXT}}', JSON.stringify(text)), context));
  const result = run(text);
  return { host, window, selection, editable, document, result, run, timeout: () => timeout(), style: () => style,
    removed: () => removed, collapsed: () => collapsed, blurred: () => blurred,
    focuses: () => focuses, styles: () => styles, pointerdown: () => listeners.get('pointerdown')?.(),
    collapsedRange: () => collapsedRange };
}

for (const tag of ['DIV', 'TEXTAREA']) {
  test(`restores selection after replacement or failure (${tag})`, () => {
    const fixture = prepare(tag);
    assert.equal(fixture.result.kind, 'draftjs');
    assert.equal(fixture.result.token, 'test-editor');
    assert.equal(fixture.result.nativeInput, tag === 'TEXTAREA');
    assert.match(fixture.style().textContent, /data-ovim-editor="test-editor"/);
    assert.match(fixture.style().textContent, /background: transparent !important/);
    assert.match(fixture.style().textContent, /caret-color: transparent !important/);
    assert.equal(fixture.collapsed(), false);
    fixture.host.__ovimInputCleanup();
    assert.equal(fixture.collapsed(), true);
    assert.equal(fixture.removed(), true);
    assert.equal(fixture.blurred(), false);
    assert.equal(fixture.host.__ovimInputCleanup, undefined);
  });
}

test('timeout restores styling if Ovim stops before cleanup', () => {
  const fixture = prepare();
  fixture.timeout();
  assert.equal(fixture.removed(), true);
  assert.equal(fixture.host.__ovimInputCleanup, undefined);
});

test('cleanup does not collapse a selection in another field', () => {
  const fixture = prepare();
  fixture.selection.anchorNode = {};
  fixture.selection.focusNode = {};
  fixture.host.__ovimInputCleanup();
  assert.equal(fixture.collapsed(), false);
  assert.equal(fixture.removed(), true);
});


test('finds a cached Reddit chat textarea through nested shadow roots', () => {
  const fixture = prepare('TEXTAREA', 4);
  assert.equal(fixture.result.kind, 'draftjs');
  assert.match(fixture.style().textContent, /background: transparent !important/);
  assert.equal(typeof fixture.window.__ovimInputCleanups['test-editor'], 'function');
  fixture.window.__ovimInputCleanups['test-editor']();
  assert.equal(fixture.collapsed(), true);
  assert.equal(fixture.removed(), true);
  assert.equal(fixture.window.__ovimInputCleanups['test-editor'], undefined);
});

test('appends trusted input without selecting the draft', () => {
  const fixture = prepare('DIV', 0, 'text追加');
  assert.equal(fixture.result.append, true);
  assert.equal(fixture.result.text, '追加');
  assert.equal(fixture.collapsedRange(), false);
});

test('edits and deletions retain whole-draft replacement', () => {
  for (const text of ['te', '', 'changed', 'a\n"quoted"']) {
    const fixture = prepare('DIV', 0, text);
    assert.equal(fixture.result.append, false);
    assert.equal(fixture.result.text, text);
    assert.equal(fixture.collapsedRange(), false);
  }
});

test('repeated updates preserve focus and suppression until session completion', () => {
  const fixture = prepare('TEXTAREA');
  const cleanup = fixture.host.__ovimInputCleanup;
  fixture.run('next');
  fixture.run('final');
  assert.equal(fixture.focuses(), 1);
  assert.equal(fixture.blurred(), false);
  assert.equal(fixture.styles(), 1);
  assert.equal(fixture.removed(), false);
  assert.equal(fixture.host.__ovimInputCleanup, cleanup);
  cleanup();
  assert.equal(fixture.removed(), true);
  assert.equal(fixture.document.activeElement, fixture.editable);
});

test('an unchanged snapshot does not touch focus, selection or styles', () => {
  const fixture = prepare('TEXTAREA', 0, 'text');
  assert.equal(fixture.result.text, '');
  assert.equal(fixture.focuses(), 0);
  assert.equal(fixture.collapsed(), false);
  assert.equal(fixture.styles(), 0);
  assert.equal(fixture.host.__ovimInputCleanup, undefined);
});

test('returning to the browser restores styling without blurring the editor', () => {
  const fixture = prepare('TEXTAREA');
  fixture.pointerdown();
  assert.equal(fixture.removed(), true);
  assert.equal(fixture.blurred(), false);
  assert.equal(fixture.host.__ovimInputCleanup, undefined);
});

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import test from 'node:test';

const source = readFileSync(new URL('../adapters/browser/get_element_rect.js', import.meta.url), 'utf8');

test('returns screen origin alongside a shadow editor viewport rectangle', () => {
  const editor = { getBoundingClientRect: () => ({ left: 25, top: 300, width: 500, height: 80 }) };
  const host = { shadowRoot: { activeElement: editor } };
  const result = JSON.parse(runInNewContext(source, {
    document: { activeElement: host, body: {}, documentElement: {} },
    window: { screenX: -1200, screenY: 39, innerHeight: 900, outerHeight: 958 },
  }));
  assert.deepEqual(result, { x: 25, y: 300, width: 500, height: 80,
    viewportHeight: 900, screenX: -1200, screenY: 39, windowHeight: 958 });
});

test('does not position against an unfocused document body', () => {
  const body = {};
  assert.equal(runInNewContext(source, { document: { activeElement: body, body } }), null);
});

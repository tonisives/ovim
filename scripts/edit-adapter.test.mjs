import assert from 'node:assert/strict';
import test from 'node:test';
import { handle, replaceText } from '../adapters/bmux/adapter.mjs';

test('unrelated applications fall through without contacting the browser', async () => {
  assert.deepEqual(await handle('capture', { bundle_id: 'com.apple.TextEdit' }, () => {
    throw new Error('Unexpected browser command');
  }), { handled: false });
});

test('capture pins the focused pane and converts its editor rectangle to screen points', async () => {
  let calls = [];
  let replies = [{ focusedClientId: 'client' }, [{ id: 'client', paneId: '%8' }],
    JSON.stringify({ found: true, text: 'ไทย\ntext', cursor: { line: 1, column: 2 } }),
    JSON.stringify({ x: 25, y: 300, width: 500, height: 80, viewportHeight: 900, screenX: -1200, screenY: 39, windowHeight: 958 }), 'x.com'];
  let result = await handle('capture', { bundle_id: 'bmux.tonis.dev' }, async args => { calls.push(args); return replies.shift(); });
  assert.deepEqual(result, { handled: true, found: true, text: 'ไทย\ntext', cursor: { line: 1, column: 2 },
    frame: { x: -1175, y: 397, width: 500, height: 80 }, hostname: 'x.com', context: { pane: '%8' } });
  assert.ok(calls.slice(2).every(args => args[2] === '%8'));
});

test('textarea updates insert multiline text without Enter keys or whole-page selection', async () => {
  let calls = [];
  let run = async args => {
    calls.push(args);
    return args[0] === 'eval' ? JSON.stringify({ kind: 'editable', nativeInput: true, append: false, text: 'a\n\nไทย', token: 'editor' }) : {};
  };
  assert.deepEqual(await replaceText(run, '%8', 'a\n\nไทย', null), { token: 'editor' });
  assert.deepEqual(calls.slice(1), [['type', '-t', '%8', '--text', 'a\n\nไทย']]);
});

test('Draft.js replacements preserve blank blocks through trusted Enter events', async () => {
  let calls = [];
  await replaceText(async args => {
    calls.push(args);
    return args[0] === 'eval' ? JSON.stringify({ kind: 'draftjs', nativeInput: false, append: false, text: 'a\n\nb', token: 'editor' }) : {};
  }, '%8', 'a\n\nb', null);
  assert.deepEqual(calls.slice(1), [
    ['key', '-t', '%8', 'Meta+A'], ['type', '-t', '%8', '--text', 'a'],
    ['key', '-t', '%8', 'Enter'], ['key', '-t', '%8', 'Enter'], ['type', '-t', '%8', '--text', 'b'],
  ]);
});

test('unchanged updates send no input and failed updates restore selection styling', async () => {
  let calls = [];
  await replaceText(async args => { calls.push(args); return JSON.stringify({ kind: 'editable', nativeInput: true, append: true, text: '', token: 'editor' }); }, '%8', 'same', 'editor');
  assert.equal(calls.length, 1);
  calls = [];
  await assert.rejects(replaceText(async args => {
    calls.push(args);
    if (args[0] === 'type') throw new Error('input failed');
    return JSON.stringify({ kind: 'editable', nativeInput: true, append: true, text: 'new', token: 'editor' });
  }, '%8', 'new', 'editor'), /input failed/);
  assert.equal(calls.at(-1)[0], 'eval');
  assert.match(calls.at(-1)[3], /__ovimInputCleanups/);
});

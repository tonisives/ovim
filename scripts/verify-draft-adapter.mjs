// Pass a dedicated blank pane and a bundled React/Draft.js fixture that exposes window.draftText.
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { gzipSync } from 'node:zlib';
import { readFile, writeFile, mkdtemp, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { findCli, handle } from '../adapters/bmux/adapter.mjs';

let [pane, fixture] = process.argv.slice(2);
if (!/^%\d+$/.test(pane ?? '') || !fixture) throw new Error('Pass a dedicated blank pane ID and fixture JS path');
let execute = promisify(execFile);
let cli = await findCli();
let run = async args => {
  let { stdout } = await execute(cli, args, { env: { HOME: process.env.HOME, PATH: process.env.PATH }, timeout: 10000 });
  let response = JSON.parse(stdout);
  assert.equal(response.ok, true, response.error);
  return response.result;
};
let evaluate = source => run(['eval', '-t', pane, source]);
assert.equal(await evaluate('location.href'), 'about:blank');
let temporary = await mkdtemp(path.join(os.tmpdir(), 'ovim-draft-loader-'));
try {
  let encoded = gzipSync(await readFile(fixture)).toString('base64');
  let loader = path.join(temporary, 'loader.js');
  await writeFile(loader, `(async () => {
    let bytes = Uint8Array.from(atob(${JSON.stringify(encoded)}), c => c.charCodeAt(0));
    let source = await new Response(new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip'))).text();
    (0, eval)(source); return true;
  })()`);
  await run(['eval', '-t', pane, '--file', loader]);
} finally {
  await rm(temporary, { recursive: true });
}
await run(['wait', '-t', pane, '--selector', '.DraftEditor-root [contenteditable="true"]']);
await evaluate(`(() => { let editor = document.querySelector('.DraftEditor-root [contenteditable="true"]'); editor.focus(); window.focusEvents = {focus:0, blur:0}; editor.addEventListener('focus',()=>window.focusEvents.focus++); editor.addEventListener('blur',()=>window.focusEvents.blur++); return true; })()`);
let capture = await handle('capture', { bundle_id: 'bmux.tonis.dev' }, args => {
  if (args[0] === 'diagnostics') return { focusedClientId: 'fixture' };
  if (args[0] === 'list-clients') return [{ id: 'fixture', paneId: pane }];
  return run(args);
});
assert.equal(capture.found, true);
let token = null;
let edits = ['first', 'first second', 'first second', 'ไทย "quote" \\', 'line 1\n\nlast line', '', 'final'];
for (let text of edits) {
  ({ token } = await handle('update', { context: capture.context, text, token }, run));
  assert.equal(await evaluate('window.draftText'), text, 'Draft.js model');
}
await handle('finish', { context: capture.context, token }, run);
assert.equal(await evaluate('Object.keys(window.__ovimInputCleanups ?? {}).length'), 0);
let events = await evaluate('window.focusEvents');
assert.equal(events.blur, 0);
assert.ok(events.focus <= 1, 'Only the initial trusted input may establish native focus');
console.log(`${edits.length} controlled Draft.js model updates passed, including blank lines, Unicode and clearing; no blur or repeated focus events.`);

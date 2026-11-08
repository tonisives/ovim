// Run against a dedicated blank background pane: node scripts/verify-edit-adapter.mjs %PANE
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { findCli, handle } from '../adapters/bmux/adapter.mjs';

let pane = process.argv[2];
if (!/^%\d+$/.test(pane ?? '')) throw new Error('Pass a dedicated blank bmux pane ID');
let cli = await findCli();
let environment = { HOME: process.env.HOME, PATH: process.env.PATH };
let execute = promisify(execFile);
let run = async args => {
  let { stdout } = await execute(cli, args, { env: environment, timeout: 10000 });
  let response = JSON.parse(stdout);
  assert.equal(response.ok, true, response.error);
  return response.result;
};
let evaluate = source => run(['eval', '-t', pane, source]);
assert.equal(await evaluate('location.href'), 'about:blank');
await evaluate(`(() => {
  document.body.innerHTML = '<input id="input"><textarea id="textarea"></textarea><div id="editable" contenteditable="true"></div><div id="host"></div>';
  let root = document.getElementById('host').attachShadow({mode:'open'});
  root.innerHTML = '<div id="inner"></div>';
  let nested = root.getElementById('inner').attachShadow({mode:'open'});
  nested.innerHTML = '<textarea id="shadow"></textarea>';
  window.editors = {
    input: document.getElementById('input'), textarea: document.getElementById('textarea'),
    editable: document.getElementById('editable'), shadow: nested.getElementById('shadow'),
  };
  window.events = { trusted: 0, enter: 0 };
  for (let editor of Object.values(window.editors)) {
    editor.addEventListener('input', event => { if(event.isTrusted) window.events.trusted++; });
    editor.addEventListener('keydown', event => { if(event.key === 'Enter') window.events.enter++; });
  }
  return true;
})()`);
let checks = 0;
for (let name of ['input', 'textarea', 'shadow', 'editable']) {
  await evaluate(`window.editors[${JSON.stringify(name)}].focus(); true`);
  let capture = await handle('capture', { bundle_id: 'bmux.tonis.dev' }, args => {
    if (args[0] === 'diagnostics') return { focusedClientId: 'fixture' };
    if (args[0] === 'list-clients') return [{ id: 'fixture', paneId: pane }];
    return run(args);
  });
  assert.equal(capture.found, true);
  assert.equal(capture.text, '');
  let token = null;
  let edits = ['hello', 'hello', 'hello ไทย', ['input', 'editable'].includes(name) ? 'quote " \\ ไทย' : 'quote " \\ ไทย\n\nlast', ''];
  for (let text of edits) {
    ({ token } = await handle('update', { context: capture.context, text, token }, run));
    let value = await evaluate(`(() => { let e = window.editors[${JSON.stringify(name)}]; return e.value ?? (e.textContent === '' ? '' : e.innerText); })()`);
    assert.equal(value, text, `${name} replacement`);
    checks++;
  }
  await handle('finish', { context: capture.context, token }, run);
  assert.equal(await evaluate('Object.keys(window.__ovimInputCleanups ?? {}).length'), 0);
}
let events = await evaluate('window.events');
assert.ok(events.trusted > 0);
assert.equal(events.enter, 0);
console.log(`${checks} browser replacements passed across input, textarea, nested shadow textarea, and contenteditable; trusted input observed; no Enter keys sent.`);

#!/usr/bin/env node
import { access, readFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHumanTransport } from './transport.mjs';

let directory = path.dirname(fileURLToPath(import.meta.url));

export let findCli = async () => {
  for (let candidate of [process.env.BMUX_BIN, '/Applications/bmux.app/Contents/Resources/bin/bmux',
    path.join(os.homedir(), 'Applications/bmux.app/Contents/Resources/bin/bmux'),
    path.join(os.homedir(), 'workspace/_tools/bmux.app/Contents/Resources/bin/bmux'),
    ...(process.env.PATH ?? '').split(path.delimiter).filter(Boolean).map(directory => path.join(directory, 'bmux'))].filter(Boolean)) {
    if (await access(candidate).then(() => true, () => false)) return candidate;
  }
  throw new Error('bmux installation not found');
};

let script = name => readFile(path.join(directory, name), 'utf8');

export let replaceText = async (run, pane, text, token) => {
  let source = (await script('prepare_input.js'))
    .replace('{{TARGET_ELEMENT_ID}}', token ?? '')
    .replace('{{NEW_TEXT}}', JSON.stringify(text));
  let prepared = JSON.parse(await run(['eval', '-t', pane, source]));
  if (!['draftjs', 'editable'].includes(prepared.kind)) throw new Error('Unsupported browser editor');
  try {
    if (prepared.append && prepared.text === '') return { token: prepared.token };
    if (!prepared.nativeInput && prepared.append) {
      await run(['cdp', '-t', pane, 'Input.dispatchKeyEvent', JSON.stringify({
        type: 'keyDown', key: 'End', code: 'End', windowsVirtualKeyCode: 35, commands: ['moveToEndOfDocument'],
      })]);
      await run(['cdp', '-t', pane, 'Input.dispatchKeyEvent', JSON.stringify({
        type: 'keyUp', key: 'End', code: 'End', windowsVirtualKeyCode: 35,
      })]);
    } else if (!prepared.nativeInput) {
      await run(['key', '-t', pane, 'Meta+A']);
    }
    if (prepared.text === '') {
      await run(['key', '-t', pane, 'Backspace']);
    } else {
      // Draft.js needs trusted Enter events to build separate blocks. Textareas
      // receive one insert so a chat textarea cannot submit on an Enter key.
      let lines = prepared.kind === 'draftjs' ? prepared.text.split('\n') : [prepared.text];
      for (let [index, line] of lines.entries()) {
        if (index > 0) await run(['key', '-t', pane, 'Enter']);
        if (line) await run(['type', '-t', pane, '--text', line]);
      }
    }
    return { token: prepared.token };
  } catch (error) {
    await finish(run, pane, prepared.token).catch(() => {});
    throw error;
  }
};

let finish = (run, pane, token) => run(['eval', '-t', pane,
  `(function() { var cleanups = window.__ovimInputCleanups; var cleanup = cleanups && cleanups[${JSON.stringify(token)}]; if (cleanup) cleanup(); return 'ok'; })()`]);

export let handle = async (operation, request, run) => {
  if (operation === 'capture') {
    if (request.bundle_id !== 'bmux.tonis.dev') return { handled: false };
    let diagnostics = await run(['diagnostics']);
    let clients = await run(['list-clients']);
    let pane = clients.find(client => client.id === diagnostics.focusedClientId)?.paneId;
    if (!pane) throw new Error('No focused bmux pane');
    let capture = JSON.parse(await run(['eval', '-t', pane, await script('../browser/get_text_and_cursor.js')]));
    let rectangle = await run(['eval', '-t', pane, await script('../browser/get_element_rect.js')]);
    let rect = typeof rectangle === 'string' ? JSON.parse(rectangle) : rectangle;
    let frame = rect && {
      x: rect.screenX + rect.x,
      y: rect.screenY + rect.windowHeight - rect.viewportHeight + rect.y,
      width: rect.width, height: rect.height,
    };
    let hostname = await run(['eval', '-t', pane, 'window.location.hostname']);
    return { handled: true, ...capture, frame, hostname, context: { pane } };
  }
  let pane = request.context?.pane;
  if (!pane) throw new Error('Missing browser target');
  if (operation === 'update') return replaceText(run, pane, request.text, request.token);
  if (operation === 'finish') { await finish(run, pane, request.token); return {}; }
  if (operation === 'cursor') {
    if (!Number.isSafeInteger(request.line) || request.line < 0 || !Number.isSafeInteger(request.column) || request.column < 0) throw new Error('Invalid cursor');
    let source = (await script('../browser/set_cursor_position.js'))
      .replace('{{TARGET_LINE}}', String(request.line)).replace('{{TARGET_COL}}', String(request.column));
    let result = await run(['eval', '-t', pane, source]);
    if (!result.startsWith('ok')) throw new Error('Could not restore cursor');
    return {};
  }
  throw new Error('Unknown edit adapter operation');
};

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    let input = '';
    for await (let chunk of process.stdin) input += chunk;
    let operation = process.argv[2], request = JSON.parse(input);
    if (operation === 'capture' && request.bundle_id !== 'bmux.tonis.dev') {
      console.log(JSON.stringify({ handled: false }));
      process.exit(0);
    }
    let cli = await findCli();
    let run = await createHumanTransport(cli);
    console.log(JSON.stringify(await handle(operation, request, run)));
  } catch {
    console.error('Browser edit adapter failed');
    process.exitCode = 1;
  }
}

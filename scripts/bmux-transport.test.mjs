import assert from 'node:assert/strict';
import { test } from 'node:test';
import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { mkdtemp, rm } from 'node:fs/promises';
import { humanCommand, connectHumanTransport } from '../adapters/bmux/transport.mjs';

test('browser edit commands carry explicit human input and preserve literal text', () => {
  assert.deepEqual(humanCommand(['type', '-t', '%4', '--text', 'Thai ไทย\n--human']), {
    method: 'type', args: { pane: '%4', text: 'Thai ไทย\n--human' }, inputSource: 'human',
  });
  assert.deepEqual(humanCommand(['key', '-t', '%4', 'Meta+A']).args, { pane: '%4', key: 'Meta+A' });
  assert.deepEqual(humanCommand(['eval', '-t', '%4', 'window.value']).args, { pane: '%4', expression: 'window.value' });
  assert.deepEqual(humanCommand(['cdp', '-t', '%4', 'Input.insertText', '{"text":"x"}']).args,
    { pane: '%4', method: 'Input.insertText', params: { text: 'x' } });
  assert.throws(() => humanCommand(['type', '-t', '%4', 'wrong']));
});

for (let supported of [true, false]) test(`socket handshake ${supported ? 'enables immediate edits' : 'rejects an older runtime'}`, async () => {
  let directory = await mkdtemp(path.join(os.tmpdir(), 'ovim-socket-'));
  let socket = path.join(directory, 'browser.sock'), commands = [];
  let server = net.createServer(connection => {
    let input = '';
    connection.setEncoding('utf8');
    connection.on('data', chunk => {
      input += chunk;
      if (!input.includes('\n')) return;
      let command = JSON.parse(input); commands.push(command);
      let response = command.method === 'input.capabilities'
        ? { ok: true, result: { human: supported } }
        : command.args.expression === 'fail' ? { ok: false, error: 'Editor disappeared' }
        : { ok: true, result: command.args };
      // Split a response at a Unicode character boundary and across chunks.
      let encoded = `${JSON.stringify(response)}\n`, midpoint = Math.floor(encoded.length / 2);
      connection.write(encoded.slice(0, midpoint));
      connection.end(encoded.slice(midpoint));
    });
  });
  await new Promise(resolve => server.listen(socket, resolve));
  try {
    if (!supported) await assert.rejects(connectHumanTransport(socket), /Reopen/);
    else {
      let run = await connectHumanTransport(socket);
      assert.deepEqual(await run(['type', '-t', '%2', '--text', 'ไทย\nnew']), { pane: '%2', text: 'ไทย\nnew' });
      await assert.rejects(run(['eval', '-t', '%2', 'fail']), /Editor disappeared/);
    }
    assert.ok(commands.every(command => command.inputSource === 'human' && !command.args?.agentId));
    assert.equal(commands[0].method, 'input.capabilities');
  } finally {
    await new Promise(resolve => server.close(resolve));
    await rm(directory, { recursive: true, force: true });
  }
});

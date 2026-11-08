import net from 'node:net';
import os from 'node:os';
import path from 'node:path';
import { realpath } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

export let humanCommand = args => {
  let [method, ...values] = args;
  let parameters = {};
  if (['diagnostics', 'list-clients'].includes(method) && !values.length) {
    return { method, args: parameters, inputSource: 'human' };
  }
  if (values[0] !== '-t' || !values[1]) throw new Error('Missing browser pane');
  parameters.pane = values[1];
  let rest = values.slice(2);
  if (method === 'eval' && rest.length === 1) parameters.expression = rest[0];
  else if (method === 'key' && rest.length === 1) parameters.key = rest[0];
  else if (method === 'type' && rest.length === 2 && rest[0] === '--text') parameters.text = rest[1];
  else if (method === 'cdp' && rest.length === 2) {
    parameters.method = rest[0]; parameters.params = JSON.parse(rest[1]);
  } else throw new Error('Unsupported browser edit command');
  return { method, args: parameters, inputSource: 'human' };
};

export let request = (socketPath, command) => new Promise((resolve, reject) => {
  let socket = net.createConnection(socketPath);
  let input = '', settled = false;
  let finish = (error, result) => {
    if (settled) return;
    settled = true;
    socket.destroy();
    if (error) reject(error); else resolve(result);
  };
  socket.setEncoding('utf8');
  socket.setTimeout(8000, () => finish(new Error('Browser edit timed out')));
  socket.on('error', error => finish(error));
  socket.on('connect', () => socket.write(`${JSON.stringify(command)}\n`));
  socket.on('data', chunk => {
    input += chunk;
    if (input.length > 8 * 1024 * 1024) return finish(new Error('Browser response too large'));
    if (!input.includes('\n')) return;
    try {
      let response = JSON.parse(input.slice(0, input.indexOf('\n')));
      if (!response.ok) throw new Error(response.error ?? 'Browser edit failed');
      finish(null, response.result);
    } catch (error) { finish(error); }
  });
  socket.on('end', () => finish(new Error('Browser closed the edit connection')));
});

export let connectHumanTransport = async socketPath => {
  let capabilities = await request(socketPath, { method: 'input.capabilities', inputSource: 'human' });
  if (!capabilities?.human) throw new Error('Reopen the updated bmux to enable human input');
  return args => request(socketPath, humanCommand(args));
};

export let createHumanTransport = async cli => {
  let directory = path.dirname(await realpath(cli));
  let { controlSocketPath } = await import(pathToFileURL(path.join(directory, 'ipc.mjs')).href);
  let { runtimeDataDirectory } = await import(pathToFileURL(path.join(directory, 'runtime-paths.mjs')).href);
  let dataDirectory = runtimeDataDirectory(process.platform, os.homedir(), process.env);
  return connectHumanTransport(controlSocketPath(dataDirectory));
};

import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';

let entry = path.resolve(process.argv[2] ?? '');
if (!process.argv[2] || path.extname(entry) !== '.mjs') throw new Error('Pass an adapter .mjs entry point');
await fs.access(entry);
let source = path.dirname(path.dirname(entry));
let config = process.env.OVIM_CONFIG_DIR ?? path.join(os.homedir(), 'Library/Application Support/ovim');
let destination = path.join(config, 'edit-adapters');
let launcher = path.join(config, 'edit-adapter');
let marker = '# Ovim edit adapter launcher';
let existing = await fs.readFile(launcher, 'utf8').catch(error => {
  if (error.code !== 'ENOENT') throw error;
  return '';
});
if (existing && !existing.includes(marker)) throw new Error('An existing custom edit-adapter is installed; add this adapter to its dispatcher');
await fs.mkdir(config, { recursive: true });
await fs.cp(source, destination, { recursive: true });
let quote = value => "'" + value.replaceAll("'", "'\\''") + "'";
let target = path.join(destination, path.relative(source, entry));
await fs.writeFile(launcher, `#!/bin/sh\n${marker}\nexec ${quote(process.execPath)} ${quote(target)} "$@"\n`, { mode: 0o755 });
await fs.chmod(launcher, 0o755);
console.log(`Installed edit adapter at ${launcher}`);

#!/usr/bin/env python3
"""Project entry point for the configured local builder; CI executes natively."""
import os
from pathlib import Path
import subprocess
import sys


def main(arguments=None):
    arguments = list(sys.argv[1:] if arguments is None else arguments)
    if not arguments or arguments[0] not in ('run', 'cargo') or '--' not in arguments:
        raise ValueError('Usage: build-local.py run|cargo [worker options] -- COMMAND [ARGS]')
    mode = arguments[0]
    separator = arguments.index('--')
    options = arguments[1:separator]
    command = arguments[separator + 1:]
    if mode == 'cargo':
        command = ['cargo', *command]
    if not command:
        raise ValueError('A finite build command is required')
    configured = (Path.home() / '.config/build-service/config.json').is_file()
    if configured and not os.environ.get('BUILD_SERVICE_WORKER') and not os.environ.get('CI'):
        executable = os.environ.get('BUILDCTL', str(Path.home() / '.agents/scripts/buildctl'))
        if not Path(executable).is_file():
            raise RuntimeError('Configured build service client is missing: ' + executable)
        os.execv(executable, [executable, *arguments])
    cwd = Path(options[options.index('--cwd') + 1]).resolve() if '--cwd' in options else Path.cwd()
    environment = dict(os.environ)
    for index, option in enumerate(options):
        if option == '--env':
            name, separator, value = options[index + 1].partition('=')
            if separator:
                environment[name] = value
            elif name not in environment:
                raise ValueError('Requested environment variable is not set: ' + name)
    if '--prepare-node' in options:
        install_root = cwd
        while not any((install_root / name).is_file() for name in ('pnpm-lock.yaml', 'package-lock.json')):
            if install_root.parent == install_root:
                raise ValueError('No pnpm or npm lockfile found above the build directory')
            install_root = install_root.parent
        install = ['pnpm', 'install', '--frozen-lockfile', '--child-concurrency=4'] if (install_root / 'pnpm-lock.yaml').is_file() else ['npm', 'ci', '--no-audit', '--no-fund']
        result = subprocess.call(install, cwd=install_root, env=environment)
        if result:
            return result
    return subprocess.call(command, cwd=cwd, env=environment)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, RuntimeError, OSError) as error:
        print('build-local: ' + str(error), file=sys.stderr)
        sys.exit(1)

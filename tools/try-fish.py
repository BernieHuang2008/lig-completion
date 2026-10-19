#!/usr/bin/env python3
"""Try the local release build in a disposable fish configuration."""
import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--debug', action='store_true', help='keep diagnostic logs in /tmp after exit')
parser.add_argument('--bin', type=Path, default=root / 'target/release/lig', help='lig executable')
args = parser.parse_args()

binary = args.bin.resolve()
fish = shutil.which('fish')
if not binary.exists() or not fish:
    sys.exit('Build with cargo build --release first, and install fish 4.2.1 or newer.')
with tempfile.TemporaryDirectory(prefix='lig-try-fish-') as directory:
    home = Path(directory)
    env = dict(os.environ, HOME=directory, XDG_CONFIG_HOME=str(home / 'config'),
               XDG_DATA_HOME=str(home / 'data'), XDG_STATE_HOME=str(home / 'state'),
               XDG_RUNTIME_DIR=str(home / 'runtime'), SHELL=fish,
               PATH=str(binary.parent) + os.pathsep + os.environ.get('PATH', ''),
               CARGO_HOME=os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')),
               RUSTUP_HOME=os.environ.get('RUSTUP_HOME', str(Path.home() / '.rustup')))
    if args.debug:
        debug = Path(tempfile.mkdtemp(prefix='lig-fish-debug-'))
        env.update(LIG_LOG='1', XDG_STATE_HOME=str(debug))
        print(f'Debug log: {debug}/lig/lig.log', flush=True)
    (home / 'runtime').mkdir(mode=0o700)
    for key in ['LIG_DISABLE', 'LIG_TTY', 'LIG_PID', 'INSIDE_EMACS', 'ZDOTDIR']:
        env.pop(key, None)
    config = home / 'config/fish/config.fish'
    config.parent.mkdir(parents=True)
    lig_config = home / 'config/lig/config.toml'
    lig_config.parent.mkdir(parents=True)
    lig_config.write_text('[behavior]\ncomplete_commands = true\n')
    config.write_text("set -g fish_greeting\nfunction fish_prompt; printf 'lig-test %s> ' (prompt_pwd); end\n")
    subprocess.run([binary, 'install', 'fish'], env=env, check=True)
    print('Temporary fish config. Try git ch or cargo bu; exit to finish.', flush=True)
    subprocess.run([fish, '-i'], env=env, check=False)
    if args.debug:
        print(f'Debug log retained: {debug}/lig/lig.log')

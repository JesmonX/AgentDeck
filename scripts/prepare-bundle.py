#!/usr/bin/env python3
"""Build and stage the native helper; Linux collector artifacts may be added by CI."""
import os, pathlib, shutil, subprocess, sys
root=pathlib.Path(__file__).resolve().parent.parent
os.chdir(root)
target=os.environ.get('TARGET') or next(line.split(': ',1)[1] for line in subprocess.check_output(['rustc','-vV'],text=True).splitlines() if line.startswith('host:'))
debug='--debug' in sys.argv
cmd=['cargo','build','--locked','-p','agentdeck-cli','--target',target]
if not debug: cmd.append('--release')
subprocess.run(cmd,check=True)
ext='.exe' if 'windows' in target else ''
binary=root/'target'/target/('debug' if debug else 'release')/('agentdeck-collector'+ext)
dest=root/'apps/desktop/src-tauri/binaries'
dest.mkdir(parents=True,exist_ok=True)
shutil.copy2(binary,dest/f'agentdeck-collector-{target}{ext}')
(root/'collectors').mkdir(exist_ok=True)
if 'linux' in target:
    folder=root/'collectors'/target
    folder.mkdir(exist_ok=True)
    shutil.copy2(binary,folder/'agentdeck-collector')
print(f'Staged {target} native helper. Linux remote collectors: '+', '.join(p.name for p in (root/'collectors').iterdir() if p.is_dir()))

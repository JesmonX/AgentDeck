#!/usr/bin/env python3
"""Stage only installable assets; require all preview platforms before publishing."""
import pathlib
import shutil
import sys

root = pathlib.Path(__file__).resolve().parent.parent
out = root / 'release-assets'
out.mkdir(exist_ok=True)
if sys.argv[1] == '--verify':
    names = [p.name for p in out.iterdir() if p.is_file() and p.stat().st_size > 0]
    for target, suffix in [('x86_64-pc-windows-msvc', '.exe'), ('x86_64-apple-darwin', '.dmg'), ('aarch64-apple-darwin', '.dmg'), ('x86_64-unknown-linux-gnu', '.deb')]:
        assert any(target in n and n.endswith(suffix) for n in names), f'Missing {target} {suffix}'
    for arch in ['x86_64', 'aarch64']:
        assert f'agentdeck-collector-{arch}-linux' in names, f'Missing collector {arch}'
    print('Verified four desktop platforms and two Linux collectors')
else:
    target = sys.argv[1]
    bundle = root / 'target' / target / 'release' / 'bundle'
    files = [p for p in bundle.rglob('*') if p.is_file() and p.suffix in ['.exe', '.msi', '.dmg', '.deb', '.rpm', '.AppImage']]
    assert files, f'No installers in {bundle}'
    for p in files:
        dest = out / f'{target}--{p.name}'
        shutil.copy2(p, dest)
        print(dest.name)

#!/usr/bin/env python3
"""Run the real Rust backend behind Vite's loopback-only development proxy."""
import os, pathlib, secrets, subprocess, time, sys
root = pathlib.Path(__file__).resolve().parent.parent
os.chdir(root)
subprocess.run(['cargo', 'build', '-p', 'agentdeck-cli'], check=True)
env = {**os.environ, 'AGENTDECK_DEV_TOKEN': secrets.token_urlsafe(36)}
binary = root / 'target/debug' / ('agentdeck-collector.exe' if os.name == 'nt' else 'agentdeck-collector')
children = []
try:
    children.append(subprocess.Popen([str(binary), 'serve'], env=env))
    time.sleep(0.3)
    if children[0].poll() is not None:
        raise SystemExit('AgentDeck 后端未启动，请检查端口 47831 是否被占用。')
    children.append(subprocess.Popen(['pnpm.cmd' if os.name == 'nt' else 'pnpm', 'web'], env=env))
    sys.exit(children[-1].wait())
except KeyboardInterrupt:
    pass
finally:
    for p in children: p.terminate()
    for p in children:
        try: p.wait(timeout=10)
        except subprocess.TimeoutExpired: p.kill()

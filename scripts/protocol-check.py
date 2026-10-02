#!/usr/bin/env python3
"""End-to-end sync test with an isolated SSH transport shim, not a live SSH host."""
import json, os, pathlib, sqlite3, subprocess, sys, tempfile
root=pathlib.Path(__file__).resolve().parent.parent
binary=root/'target/debug/agentdeck-collector'
subprocess.run(['cargo','build','-p','agentdeck-cli','--locked'],cwd=root,check=True)
with tempfile.TemporaryDirectory(prefix='agentdeck-sync-') as temp:
    temp=pathlib.Path(temp);remote=temp/'remote';local=temp/'local';logs=temp/'logs';logs.mkdir();bin_dir=temp/'bin';bin_dir.mkdir()
    shim=bin_dir/'ssh'
    shim.write_text(f'#!{sys.executable}\nimport os,sys\nassert sys.argv[-1]=="$HOME/.local/lib/agentdeck/agentdeck-collector rpc"\ne=dict(os.environ);e["AGENTDECK_DATA_DIR"]=e["AGENTDECK_TEST_REMOTE"]\nb=e["AGENTDECK_TEST_BINARY"]\nos.execve(b,[b,"rpc"],e)\n')
    shim.chmod(0o700)
    env={**os.environ,'PATH':str(bin_dir)+os.pathsep+os.environ['PATH'],'AGENTDECK_TEST_REMOTE':str(remote),'AGENTDECK_TEST_BINARY':str(binary)}
    def run(state,mode,*args,p=None):
        result=subprocess.run([str(binary),mode,*args],input=json.dumps(p or {}),env={**env,'AGENTDECK_DATA_DIR':str(state)},capture_output=True,text=True,timeout=60,check=True)
        return json.loads(result.stdout)
    remote_settings=run(remote,'command','settings');remote_settings['sources']=[{'id':'fixture','agent':'claude','path':str(logs),'enabled':True}];remote_settings['accounts']=[]
    run(remote,'command','saveSettings',p=remote_settings)
    log=logs/'session.jsonl'
    def line(i):return json.dumps({'type':'assistant','sessionId':'replicated-session','timestamp':'2026-10-01T10:00:00Z','message':{'id':f'response-{i}','model':'test-model','usage':{'input_tokens':10,'output_tokens':5,'cache_read_input_tokens':20,'cache_creation_input_tokens':0}}})+'\n'
    log.write_text(''.join(line(i) for i in range(505)))
    run(remote,'scan');run(remote,'sample')
    settings=run(local,'command','settings');settings['sources']=[];settings['accounts']=[];settings['servers']=[{'id':'fixture','label':'Fixture','host':'fixture-host'}]
    run(local,'command','saveSettings',p=settings)
    def sync(expected):
        state=run(local,'command','serverSync',p={'id':'fixture'});assert state['status']=='ready',state
        snapshot=run(local,'snapshot');assert sum(r['requests'] for r in snapshot['usage'])==expected
        assert sum(r['input']+r['output'] for r in snapshot['usage'])==expected*35
    sync(505);sync(505)
    with log.open('a') as f:f.write(line(505))
    run(remote,'scan');sync(506)
    c=sqlite3.connect(remote/'agentdeck.db');watermark=c.execute("select seq from sqlite_sequence where name='changes'").fetchone()[0];c.execute('delete from changes');c.execute("insert or replace into kv values('journal_pruned_through',?)",[json.dumps(watermark+1)]);c.commit();c.close()
    sync(506)
    print('PASS: 505-row paginated bootstrap, idempotent replay, incremental append, journal-gap recovery, sample transfer (isolated transport shim).')

#!/usr/bin/env python3
"""M5: synthetic argv only; evidence contains counts/states, never argv/values."""
# SPDX-License-Identifier: MIT
import json
import os
import select
import socket
import subprocess
import sys
import time
from connect_monitor import Monitor, run
from connect_policy import Server, control
from tui_monitor import Terminal

CLIENT = r'''
import json,os,socket,sys
from pathlib import Path
stat=Path('/proc/self/stat').read_text().rsplit(')',1)[1].split()
print(json.dumps({'pid':os.getpid(),'ticks':int(stat[19])}),flush=True)
for line in sys.stdin:
    port=int(line)
    with socket.socket() as s:
        s.settimeout(2);code=0
        try:
            s.connect(('127.0.0.1',port));s.sendall(b'warden-m5-synthetic');assert s.recv(64)==b'warden-m5-synthetic'
        except OSError as e:code=e.errno
    print(json.dumps({'errno':code}),flush=True)
'''
SERIAL=0
class Process:
    def __init__(self, args, inside=True):
        global SERIAL
        SERIAL+=1;self.unit=f'warden-m5-{time.monotonic_ns()}-{SERIAL}'
        command=['python3','-u','-c',CLIENT,*args]
        if any(isinstance(a,bytes) for a in args):
            # D-Bus requires UTF-8; re-exec inside the already-scoped process.
            wrapper='import os,sys; os.execv(sys.executable,'+repr(['python3','-u','-c',CLIENT,*args])+')'
            command=['python3','-c',wrapper]
        self.p=subprocess.Popen(['systemd-run' ,'--quiet','--wait','--pipe','--collect',f'--unit={self.unit}','--description=veil-warden synthetic argv fixture','--property=LimitCORE=0','--slice=warden-test.slice' if inside else '--slice=system.slice',*command],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
        try:
            self.identity=self.read();self.target=f'{self.identity["pid"]}:{self.identity["ticks"]}'
        except Exception:
            subprocess.run(['systemctl','stop',self.unit],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
            self.close();raise
    def read(self):
        if not select.select([self.p.stdout],[],[],5)[0]:raise AssertionError('synthetic process timeout')
        return json.loads(self.p.stdout.readline())
    def send(self, port):
        self.p.stdin.write(f'{port}\n');self.p.stdin.flush();return self.read()['errno']
    def close(self):
        if self.p.poll() is None:
            self.p.stdin.close();self.p.wait(timeout=5)
        for f in [self.p.stdout,self.p.stderr]:f.close()

def check(condition, label):
    if not condition:raise AssertionError(label)

def scan(binary,obj,target,expected,markers):
    m=Monitor(binary,obj,extra=['--scan-argv',target])
    try:
        lines=[s for s in m.lines if s.startswith('scan ')]
        check(len(lines)==1, 'one scan only')
        check(f'argv_scan={expected}' in lines[0], 'evaluation state')
        check(not any(marker in '\n'.join(m.lines) for marker in markers), 'CLI value redaction')
        m.stop('term')
        check(not any(marker in '\n'.join(m.lines) for marker in markers), 'final CLI value redaction')
        return lines[0]
    finally:m.cleanup()

def main():
    binary,obj=sys.argv[1:];run('systemctl','start','warden-test.slice')
    # Format-valid but intentionally fake and never used for authentication.
    fake='ghp_'+'A'*36;marker='M5_PRIVATE_ARG_NOT_FOR_OUTPUT'
    p=Process([fake,marker]);s=Server(socket.AF_INET,'127.0.0.1');records={}
    try:
        m=Monitor(binary,obj,extra=['--enforce','--scan-argv',p.target])
        try:
            line=next(s for s in m.lines if s.startswith('scan '))
            check('argv_scan=evaluated count=1' in line and 'creds.github.pat.ghp:1' in line and 'warning=possible_secret' in line, 'real pinned rule detection')
            check('mode=enforce rules=0' in control(binary,'list'),'no automatic policy mutation')
            check(p.send(s.port)==0,'warned process still allowed')
            m.wait(lambda:len(m.events())==1)
            check(m.events()[0]['decision']=='allow','warned event allow')
            check('mode=enforce rules=0' in control(binary,'list'),'post-event map unchanged')
            m.stop('term');text='\n'.join(m.lines)
            check(fake not in text and marker not in text,'CLI secrets absent')
            records['detection']=line;records['warned_connection_allowed']=True;records['deny_map_unchanged']=True
        finally:m.cleanup()
        t=Terminal([binary,'connect','--object',obj,'--tui','--enforce','--scan-argv',p.target])
        try:
            t.wait(lambda:'warning=possible_secret' in t.text())
            check('creds.github.pat.ghp:1' in t.text(),'TUI rule ID')
            check(fake.encode() not in t.raw and marker.encode() not in t.raw,'PTY value redaction')
            check(p.send(s.port)==0,'TUI warned connection allowed')
            t.wait(lambda:f'127.0.0.1:{s.port}' in t.text())
            t.key(b'q');result=t.finish();check(fake.encode() not in t.raw and marker.encode() not in t.raw,'final PTY redaction')
            records['tui']={k:v for k,v in result.items() if k!='snapshots'}
        finally:t.close()
        wrong=f'{p.identity["pid"]}:{p.identity["ticks"]+1}'
        records['identity_mismatch']=scan(binary,obj,wrong,'identity_changed',[fake,marker])
        p.close();records['gone']=scan(binary,obj,p.target,'gone',[fake,marker])
        for name,args,inside,state in [('normal',['ordinary-words'],True,'evaluated'),('oversized',['B'*20000],True,'too_large'),('invalid_utf8',[b'\xff'],True,'invalid_utf8'),('outside',[fake,marker],False,'out_of_scope')]:
            target=Process(args,inside)
            try:records[name]=scan(binary,obj,target.target,state,[fake,marker])
            finally:target.close()
        check('count=0' in records['normal'] and 'warning=no_match' in records['normal'],'normal input difference')
        # Inspect only the test unit's journal; never print the journal or matched text.
        journal=run('journalctl','--no-pager','-o','json','-u',p.unit)
        check(fake not in journal and marker not in journal,'journal redaction')
        records['journal_values_absent']=True
        records['kernel']=run('uname','-r');records['ssh_active']=run('systemctl','is-active','sshd.service')=='active'
        print(json.dumps(records,ensure_ascii=False,indent=2))
    finally:p.close();s.close()
if __name__=='__main__':main()

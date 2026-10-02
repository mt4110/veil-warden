#!/usr/bin/env python3
"""M2 TCP connect attempts and loss accounting, in the dedicated VM only."""
# SPDX-License-Identifier: MIT
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import threading
import time

SCOPE = "/sys/fs/cgroup/warden.slice/warden-test.slice"

def run(*args):
    return subprocess.check_output(args, text=True, timeout=20).strip()

def links():
    return json.loads(run("bpftool", "-j", "link", "show"))

def attached():
    return json.loads(run("bpftool", "-j", "cgroup", "show", SCOPE))

def signatures(items):
    return sorted(json.dumps({k:v for k,v in x.items() if k != "id"}, sort_keys=True) for x in items)

def fields(line):
    return dict(p.split("=",1) for p in line.split()[1:])

CLIENT = r'''
import json, os, socket, threading, time, sys
from pathlib import Path
inside, family, address, port, kind, count, linger = sys.argv[1:]
scope = Path('/proc/self/cgroup').read_text().strip().split(':',2)[2]
assert scope.startswith('/warden.slice/warden-test.slice/') == (inside=='inside'), scope
result = {'pid':os.getpid(),'scope':scope,'cgroup_id':os.stat('/sys/fs/cgroup'+scope).st_ino,'family':int(family),'address':address,'port':int(port),'kind':kind,'count':int(count)}
def work():
    result['tid'] = threading.get_native_id()
    errors=0
    for _ in range(int(count)):
        with socket.socket(int(family),socket.SOCK_DGRAM if kind=='udp' else socket.SOCK_STREAM) as s:
            s.settimeout(2)
            try:
                s.connect((address,int(port)))
                if kind=='success': s.sendall(b'warden-m2-synthetic')
            except ConnectionRefusedError: errors += 1
    result['refused']=errors
thread=threading.Thread(target=work); thread.start(); thread.join()
print(json.dumps(result),flush=True)
time.sleep(float(linger))
'''

class Monitor:
    def __init__(self,binary,obj,duration=0,delay=0):
        self.before=links(); self.baseline=attached(); self.lines=[]
        self.process=subprocess.Popen([binary,'connect','--object',obj,'--interval-ms','100','--duration-ms',str(duration),'--reader-delay-ms',str(delay)], text=True,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        self.reader=threading.Thread(target=self.read,daemon=True); self.reader.start()
        self.wait(lambda: any(x.startswith('attached ') for x in self.lines))
        self.live=links(); self.new={x['id'] for x in self.live}-{x['id'] for x in self.before}
        assert len(self.new)==2, self.live
        assert {x['attach_type'] for x in self.live if x['id'] in self.new}=={'cgroup_inet4_connect','cgroup_inet6_connect'}
    def read(self):
        for line in self.process.stdout: self.lines.append(line.strip())
    def wait(self,predicate,timeout=5):
        deadline=time.monotonic()+timeout
        while not predicate():
            if self.process.poll() is not None: raise AssertionError(self.process.stderr.read())
            if time.monotonic()>=deadline: raise AssertionError('monitor timeout')
            time.sleep(.02)
    def events(self): return [fields(x) for x in self.lines if x.startswith('attempt ')]
    def stats(self): return [fields(x) for x in self.lines if x.startswith('stats ')]
    def stop(self,mode):
        if mode=='term': self.process.send_signal(signal.SIGTERM)
        if mode=='kill': self.process.send_signal(signal.SIGKILL)
        code=self.process.wait(timeout=10);self.reader.join(timeout=2)
        assert code==(-signal.SIGKILL if mode=='kill' else 0), (code,self.process.stderr.read())
        assert {x['id'] for x in links()}=={x['id'] for x in self.before}
        assert signatures(attached())==signatures(self.baseline)
        assert run('systemctl','is-active','sshd.service')=='active'
        self.process.stdout.close();self.process.stderr.close()
        return code
    def cleanup(self):
        if self.process.poll() is None:
            self.process.terminate();self.process.wait(timeout=5)

serial=0

def client(family,address,port,kind='success',inside=True,count=1,linger=.2):
    global serial
    serial+=1
    text=run('systemd-run','--quiet','--wait','--pipe','--collect',f'--unit=warden-m2-client-{serial}',
             '--slice=warden-test.slice' if inside else '--slice=system.slice',
             'python3','-c',CLIENT,'inside' if inside else 'outside',str(family),address,str(port),kind,str(count),str(linger))
    return json.loads(text)

def servers():
    result=[]; received=[]; errors=[]
    def accept(server):
        try:
            while True:
                c,_=server.accept()
                with c:
                    if c.recv(64)!=b'warden-m2-synthetic': raise AssertionError('unexpected synthetic data')
                    received.append(server.family)
        except (OSError,TimeoutError): pass
        except Exception as e: errors.append(str(e))
    for family,address in [(socket.AF_INET,'127.0.0.1'),(socket.AF_INET6,'::1')]:
        s=socket.socket(family,socket.SOCK_STREAM);s.settimeout(3);s.bind((address,0));s.listen(8)
        t=threading.Thread(target=accept,args=(s,),daemon=True);t.start();result.append(s)
    return result,received,errors

def basic(binary,obj,mode):
    m=Monitor(binary,obj,6000 if mode=='normal' else 0)
    ss,received,errors=servers(); records=[]
    try:
        for s in ss:
            address=s.getsockname()[0]; port=s.getsockname()[1]
            c=client(s.family,address,port);records.append(c)
            m.wait(lambda: any(int(e['tgid'])==c['pid'] for e in m.events()))
            e=next(e for e in m.events() if int(e['tgid'])==c['pid'])
            expected=f'{address}:{port}' if s.family==socket.AF_INET else f'[{address}]:{port}'
            assert e['destination']==expected and int(e['tid'])==c['tid'] and int(e['cgroup'])==c['cgroup_id'], (c,e)
            assert c['tid']!=c['pid'] and c['refused']==0
            assert e['connection_result']=='unknown' and e['decision']=='allow'
        # Refused TCP connect remains an observed attempt; connected UDP is out of scope.
        closed=socket.socket(socket.AF_INET,socket.SOCK_STREAM);closed.bind(('127.0.0.1',0));port=closed.getsockname()[1];closed.close()
        c=client(socket.AF_INET,'127.0.0.1',port,'refused');records.append(c)
        assert c['refused']==1
        m.wait(lambda:any(int(e['tgid'])==c['pid'] for e in m.events()))
        for s in ss: records.append(client(s.family,s.getsockname()[0],s.getsockname()[1],inside=False))
        records.append(client(socket.AF_INET,'127.0.0.1',port,'udp'))
        m.wait(lambda:bool(m.stats()) and int(m.stats()[-1]['attempted'])==3)
        assert len(m.events())==3, m.events()
        assert sorted(received)==[socket.AF_INET,socket.AF_INET,socket.AF_INET6,socket.AF_INET6] and not errors
        current={x['id'] for x in links()}
        duplicate=subprocess.run([binary,'connect','--object',obj,'--duration-ms','20'],capture_output=True,text=True,timeout=5)
        assert duplicate.returncode!=0 and {x['id'] for x in links()}==current
        code=m.stop(mode)
        final=m.stats()[-1] if mode!='kill' else None
        if final:
            assert final['final']=='true' and int(final['attempted'])==3 and int(final['ring_dropped'])==0 and int(final['decode_errors'])==0 and int(final['queue_dropped'])==0
        return {'exit':mode,'returncode':code,'events':m.events(),'clients':records,'final_stats':final,'links_detached':True,'ssh_active':True}
    finally:
        m.cleanup()
        for s in ss:s.close()

def overload(binary,obj):
    m=Monitor(binary,obj,0,800)
    try:
        closed=socket.socket(socket.AF_INET,socket.SOCK_STREAM);closed.bind(('127.0.0.1',0));port=closed.getsockname()[1];closed.close()
        rss_before=Path(f'/proc/{m.process.pid}/status').read_text().split('VmRSS:')[1].splitlines()[0].strip()
        clients=[client(socket.AF_INET,'127.0.0.1',port,'refused',count=4096,linger=0) for _ in range(2)]
        assert all(c['refused']==4096 for c in clients)
        m.wait(lambda:bool(m.stats()) and int(m.stats()[-1]['attempted'])==8192,timeout=8)
        time.sleep(1)
        rss_after=Path(f'/proc/{m.process.pid}/status').read_text().split('VmRSS:')[1].splitlines()[0].strip()
        m.stop('term'); final=m.stats()[-1]
        assert int(final['ring_dropped'])>0 and int(final['attempted'])==8192
        assert int(final['attempted'])==int(final['emitted'])+int(final['ring_dropped'])
        assert int(final['decoded'])==int(final['displayed'])+int(final['queue_dropped'])
        assert int(final['decode_errors'])==0
        assert any(e['comm_status']=='unavailable' for e in m.events()), 'short-lived process not exercised'
        return {'clients':clients,'final_stats':final,'rss_before':rss_before,'rss_after':rss_after,'short_lived_comm_unavailable':True,'links_detached':True}
    finally:m.cleanup()

def main():
    assert os.geteuid()==0 and Path('/etc/hostname').read_text().strip()=='veil-warden-sandbox'
    run('systemctl','start','warden-test.slice')
    binary,obj,partial=sys.argv[1:]
    before=links(); baseline=attached()
    failed=subprocess.run([binary,'connect','--object',partial,'--duration-ms','20'],capture_output=True,text=True,timeout=5)
    assert failed.returncode!=0 and 'connect program missing' in failed.stderr
    assert links()==before and signatures(attached())==signatures(baseline)
    print(json.dumps({'kernel':run('uname','-r'),'architecture':run('uname','-m'),
                     'partial_startup':{'rejected':True,'links_detached':True},
                     'cases':[basic(binary,obj,mode) for mode in ['normal','term','kill']],
                     'overload':overload(binary,obj)},indent=2))

if __name__=='__main__':main()

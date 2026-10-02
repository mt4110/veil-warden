#!/usr/bin/env python3
"""M3 scoped TCP policy, synthetic loopback traffic only."""
# SPDX-License-Identifier: MIT
import errno
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import threading
import time
from connect_monitor import Monitor, attached, links, run, signatures

SERIAL=0
CLIENT=r'''
import json,os,socket,sys
from pathlib import Path
family,address,port,kind=sys.argv[1:]
scope=Path('/proc/self/cgroup').read_text().strip().split(':',2)[2]
with socket.socket(int(family),socket.SOCK_DGRAM if kind=='udp' else socket.SOCK_STREAM) as s:
    s.settimeout(2)
    code=0
    try:
        s.connect((address,int(port)))
        s.sendall(b'warden-m3')
        if kind!='udp': assert s.recv(64)==b'warden-m3'
    except OSError as e: code=e.errno
print(json.dumps({'pid':os.getpid(),'scope':scope,'errno':code,'family':int(family),'address':address,'port':int(port)}),flush=True)
'''

def client(family,address,port,inside=True,kind='tcp'):
    global SERIAL
    SERIAL+=1
    return json.loads(run('systemd-run','--quiet','--wait','--pipe','--collect',f'--unit=warden-m3-{SERIAL}',
        '--slice=warden-test.slice' if inside else '--slice=system.slice','python3','-c',CLIENT,str(family),address,str(port),kind))

class Server:
    def __init__(self,family,address,kind=socket.SOCK_STREAM,port=0):
        self.s=socket.socket(family,kind);self.s.settimeout(.1);self.s.bind((address,port));self.received=[];self.errors=[];self.done=False
        if kind==socket.SOCK_STREAM:self.s.listen(32)
        self.family=family;self.address=address;self.port=self.s.getsockname()[1]
        def serve():
            while not self.done:
                try:
                    if kind==socket.SOCK_DGRAM:
                        b,_=self.s.recvfrom(64);self.received.append(b.decode());continue
                    c,_=self.s.accept()
                    def echo(c):
                        with c:
                            c.settimeout(10)
                            while True:
                                b=c.recv(64)
                                if not b:break
                                self.received.append(b.decode());c.sendall(b)
                    threading.Thread(target=echo,args=(c,),daemon=True).start()
                except socket.timeout:pass
                except OSError:
                    if not self.done: self.errors.append('server socket error')
        self.thread=threading.Thread(target=serve,daemon=True);self.thread.start()
    def close(self):self.done=True;self.thread.join(timeout=1);self.s.close()

def control(binary,*args,ok=True):
    r=subprocess.run([binary,'policy',*map(str,args)],capture_output=True,text=True,timeout=5)
    assert (r.returncode==0)==ok,(args,r.returncode,r.stdout,r.stderr)
    assert r.stdout.startswith('ok ' if ok else 'error '),(args,r.stdout)
    return r.stdout

def assert_deny(m,c):
    assert c['errno']==errno.EPERM,c
    m.wait(lambda:any(int(e['tgid'])==c['pid'] for e in m.events()))
    e=next(e for e in m.events() if int(e['tgid'])==c['pid'])
    assert e['decision']=='deny' and int(e['policy_id'])>0,e
    return e

def basic(binary,obj,mode):
    s4=Server(socket.AF_INET,'127.0.0.1');s6=Server(socket.AF_INET6,'::1');other=Server(socket.AF_INET,'127.0.0.1');other_ip=Server(socket.AF_INET,'127.0.0.2',port=s4.port)
    udp=Server(socket.AF_INET,'127.0.0.1',socket.SOCK_DGRAM,s4.port)
    m=Monitor(binary,obj,7000 if mode=='normal' else 0,extra=['--enforce','--deny',s4.address,str(s4.port)])
    records=[]
    try:
        assert 'mode=enforce rules=1' in control(binary,'list')
        c=client(s4.family,s4.address,s4.port);records.append(c);assert_deny(m,c)
        control(binary,'add',s6.address,s6.port)
        for family,address,port in [(s6.family,s6.address,s6.port),(socket.AF_INET6,'::ffff:127.0.0.1',s4.port)]:
            c=client(family,address,port);records.append(c);assert_deny(m,c)
        time.sleep(.1);assert not s4.received and not s6.received
        for server,inside in [(other,True),(other_ip,True),(s4,False),(s6,False)]:
            c=client(server.family,server.address,server.port,inside);records.append(c);assert c['errno']==0,c
        c=client(socket.AF_INET,'127.0.0.1',s4.port,kind='udp');assert c['errno']==0
        time.sleep(.1);assert udp.received==['warden-m3']
        # Only in-scope TCP appears; address, port and protocol are distinct.
        assert len(m.events())==5,m.events()
        control(binary,'add',s4.address,s4.port,ok=False)
        control(binary,'remove','127.0.0.1',1,ok=False)
        control(binary,'remove',s4.address,s4.port);control(binary,'remove',s6.address,s6.port)
        for s in [s4,s6]:assert client(s.family,s.address,s.port)['errno']==0
        # Keep both denies active until each exit mode; then prove recovery.
        control(binary,'add',s4.address,s4.port);control(binary,'add',s6.address,s6.port)
        assert_deny(m,client(s4.family,s4.address,s4.port))
        assert_deny(m,client(s6.family,s6.address,s6.port))
        m.stop(mode)
        restored=[client(s.family,s.address,s.port) for s in [s4,s6]]
        assert all(c['errno']==0 for c in restored),restored
        assert all(not s.errors for s in [s4,s6,other,other_ip,udp])
        absent=subprocess.run([binary,'policy','list'],capture_output=True,text=True,timeout=5);assert absent.returncode!=0
        return {'exit':mode,'denied_errno':errno.EPERM,'denied_receiver_packets':0,'mapped_ipv6_denied':True,'unmatched_address_port_udp_and_outside_allowed':True,'remove_and_exit_restore_both_families':True,'control_endpoint_closed':True,'links_detached':True,'ssh_active':True,'events':m.events(),'clients':records,'restored_clients':restored,'final_stats':m.stats()[-1] if mode!='kill' else None}
    finally:
        m.cleanup()
        for s in [s4,s6,other,other_ip,udp]:s.close()

PERSIST=r'''
import socket,sys
s=socket.socket();s.connect(('127.0.0.1',int(sys.argv[1])))
for line in sys.stdin:
    s.sendall(b'warden-m3');assert s.recv(64)==b'warden-m3';print('received',flush=True)
s.close()
'''
def existing_and_capacity(binary,obj):
    s=Server(socket.AF_INET,'127.0.0.1');m=Monitor(binary,obj,extra=['--enforce'])
    p=subprocess.Popen(['systemd-run','--quiet','--wait','--pipe','--collect','--unit=warden-m3-persistent','--slice=warden-test.slice','python3','-u','-c',PERSIST,str(s.port)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        def exchange():
            p.stdin.write('send\n');p.stdin.flush()
            # select keeps a broken persistent connection from hanging the suite.
            import select
            assert select.select([p.stdout],[],[],3)[0]
            assert p.stdout.readline().strip()=='received'
        exchange();control(binary,'add',s.address,s.port);exchange()
        assert client(s.family,s.address,s.port)['errno']==errno.EPERM
        for port in range(10000,10015):control(binary,'add','127.0.0.1',port)
        failed=control(binary,'add','127.0.0.1',10015,ok=False)
        assert 'rules=16 capacity=16' in failed
        state=control(binary,'list');assert 'rules=16 capacity=16' in state and f'destination=127.0.0.1:{s.port} ' in state
        assert 'destination=127.0.0.1:10015 ' not in state
        control(binary,'remove','127.0.0.1',10000);control(binary,'add','127.0.0.1',10015)
        assert 'rules=16 capacity=16' in control(binary,'list')
        exchange();p.stdin.close();assert p.wait(timeout=5)==0,p.stderr.read()
        # Losing the ring must not allow a denied destination.
        m.stop('term')
        return {'existing_connection_survived_three_exchanges':True,'capacity_failure':failed,'capacity_state_preserved':True,'remove_frees_capacity':True}
    finally:
        if p.poll() is None:p.terminate();p.wait(timeout=5)
        m.cleanup();s.close()

def overload(binary,obj):
    s=Server(socket.AF_INET,'127.0.0.1')
    m=Monitor(binary,obj,delay=800,extra=['--enforce','--deny',s.address,str(s.port)])
    code="""import socket,sys,json,errno
errors=[]
for _ in range(4096):
    with socket.socket() as s:
        try:s.connect(('127.0.0.1',int(sys.argv[1])))
        except OSError as e:errors.append(e.errno)
assert len(errors)==4096 and set(errors)=={errno.EPERM},errors
print(json.dumps({'denied':len(errors),'errno':errors[0]}))
"""
    try:
        result=json.loads(run('systemd-run','--quiet','--wait','--pipe','--collect','--unit=warden-m3-overload','--slice=warden-test.slice','python3','-c',code,str(s.port)))
        m.wait(lambda: bool(m.stats()) and int(m.stats()[-1]['attempted'])==4096,timeout=8)
        m.stop('term');stats=m.stats()[-1]
        assert int(stats['ring_dropped'])>0 and int(stats['denied'])==4096
        assert int(stats['attempted'])==int(stats['emitted'])+int(stats['ring_dropped'])
        assert not s.received
        return {'client':result,'final_stats':stats,'receiver_packets':0,'loss_does_not_change_verdict':True}
    finally:m.cleanup();s.close()

def main():
    assert os.geteuid()==0 and Path('/etc/hostname').read_text().strip()=='veil-warden-sandbox'
    run('systemctl','start','warden-test.slice');binary,obj,partial=sys.argv[1:]
    # Observe is default and updates are explicitly rejected.
    m=Monitor(binary,obj)
    try:
        assert 'mode=observe rules=0' in control(binary,'list');control(binary,'add','127.0.0.1',443,ok=False)
        unprivileged=subprocess.run(['runuser','-u','warden','--',binary,'policy','add','127.0.0.1','443'],capture_output=True,text=True,timeout=5)
        assert unprivileged.returncode!=0 and not unprivileged.stdout
        assert 'rules=0' in control(binary,'list')
        for request in [b'\xff',b'x'*257]:
            with socket.socket(socket.AF_UNIX) as stream:
                stream.settimeout(3);stream.connect('\0veil-warden-policy');stream.sendall(request);stream.shutdown(socket.SHUT_WR)
                response=b''
                while True:
                    chunk=stream.recv(4096)
                    if not chunk:break
                    response+=chunk
                assert response.startswith(b'error ') and b'rules=0' in response
        with socket.socket(socket.AF_UNIX) as stream:
            stream.settimeout(3);stream.connect('\0veil-warden-policy')
            assert stream.recv(1)==b'', 'server must time out an unfinished request'
        assert 'rules=0' in control(binary,'list')
        m.stop('term')
    finally:m.cleanup()
    # An unprivileged process occupying the endpoint cannot forge success replies.
    fake=subprocess.Popen(['runuser','-u','warden','--','python3','-u','-c',"import socket,time; s=socket.socket(socket.AF_UNIX);s.bind('\\0veil-warden-policy');s.listen();print('ready',flush=True);c,_=s.accept();time.sleep(.3)"],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    try:
        import select
        assert select.select([fake.stdout],[],[],3)[0] and fake.stdout.readline().strip()=='ready'
        rejected=subprocess.run([binary,'policy','list'],capture_output=True,text=True,timeout=5)
        assert rejected.returncode!=0 and 'policy server must be root' in rejected.stderr
        assert fake.wait(timeout=5)==0,fake.stderr.read()
    finally:
        if fake.poll() is None:fake.terminate();fake.wait(timeout=5)
    before=links();baseline=attached();s=Server(socket.AF_INET,'127.0.0.1')
    try:
        fail=subprocess.run([binary,'connect','--object',partial,'--enforce','--deny',s.address,str(s.port),'--duration-ms','20'],capture_output=True,text=True,timeout=5)
        assert fail.returncode!=0 and 'connect program missing' in fail.stderr
        assert links()==before and signatures(attached())==signatures(baseline)
        assert client(s.family,s.address,s.port)['errno']==0
    finally:s.close()
    print(json.dumps({'kernel':run('uname','-r'),'architecture':run('uname','-m'),'observe_updates_rejected':True,'non_root_control_rejected':True,'unprivileged_fake_server_rejected':True,'malformed_oversized_and_stalled_control_bounded':True,'partial_startup_links_detached_and_connection_restored':True,'cases':[basic(binary,obj,x) for x in ['normal','term','kill']],'existing_and_capacity':existing_and_capacity(binary,obj),'overload':overload(binary,obj)},indent=2))
if __name__=='__main__':main()

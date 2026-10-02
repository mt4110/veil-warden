#!/usr/bin/env python3
"""Linux PTY acceptance: real input, kernel decisions, and terminal restoration."""
# SPDX-License-Identifier: MIT
import errno
import codecs
import fcntl
import json
import os
import pty
import re
import select
import signal
import struct
import subprocess
import sys
import termios
import time
import unicodedata
from connect_monitor import attached, links, run, signatures, client as burst_client
from connect_policy import Server, client, control
import socket

class Screen:
    def __init__(self,cols=120,rows=32):self.reset(cols,rows);self.alternate=False;self.buffer=''
    def reset(self,cols,rows):self.cols=cols;self.rows=rows;self.cells=[[' ']*cols for _ in range(rows)];self.x=0;self.y=0
    def feed(self,text):
        self.buffer+=text
        while self.buffer:
            if self.buffer[0]=='\x1b':
                if len(self.buffer)<2:return
                if self.buffer[1]=='[':
                    match=re.match(r'\x1b\[([0-9;?]*)([A-Za-z~])',self.buffer)
                    if not match:
                        if len(self.buffer)<30:return
                        self.buffer=self.buffer[1:];continue
                    args,code=match.groups();self.buffer=self.buffer[match.end():]
                    nums=[int(x or 0) for x in args.lstrip('?').split(';')];n=nums[0] or 1
                    if code in 'Hf':self.y=(nums[0] or 1)-1;self.x=(nums[1] or 1)-1 if len(nums)>1 else 0
                    elif code=='A':self.y=max(0,self.y-n)
                    elif code=='B':self.y=min(self.rows-1,self.y+n)
                    elif code=='C':self.x=min(self.cols-1,self.x+n)
                    elif code=='D':self.x=max(0,self.x-n)
                    elif code=='G':self.x=n-1
                    elif code=='J' and nums[0]==2:self.reset(self.cols,self.rows)
                    elif code=='K' and self.y<self.rows:
                        start=0 if nums[0] in (1,2) else self.x
                        end=self.cols if nums[0] in (0,2) else self.x+1
                        for x in range(max(0,start),min(end,self.cols)):self.cells[self.y][x]=' '
                    elif code in 'hl' and args=='?1049':self.alternate=code=='h'
                    continue
                self.buffer=self.buffer[2:];continue
            c=self.buffer[0];self.buffer=self.buffer[1:]
            if c=='\r':self.x=0
            elif c=='\n':self.y=min(self.rows-1,self.y+1)
            elif c=='\b':self.x=max(0,self.x-1)
            elif ord(c)>=32:
                width=2 if unicodedata.east_asian_width(c) in 'WF' else 1
                if 0<=self.y<self.rows and 0<=self.x<self.cols:
                    if self.cells[self.y][self.x]=='' and self.x>0:self.cells[self.y][self.x-1]=' '
                    if self.x+1<self.cols and self.cells[self.y][self.x+1]=='':self.cells[self.y][self.x+1]=' '
                    self.cells[self.y][self.x]=c
                    if width==2 and self.x+1<self.cols:self.cells[self.y][self.x+1]=''
                self.x+=width
    def text(self):return '\n'.join(''.join(row).rstrip() for row in self.cells)

class Terminal:
    def __init__(self,args):
        self.before=links();self.baseline=attached();self.master,self.slave=pty.openpty();self.original=termios.tcgetattr(self.slave);self.screen=Screen();self.raw=b'';self.snapshots={};self.decoder=codecs.getincrementaldecoder('utf-8')()
        fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',32,120,0,0))
        def session():os.setsid();fcntl.ioctl(0,termios.TIOCSCTTY,0)
        self.process=subprocess.Popen(args,stdin=self.slave,stdout=self.slave,stderr=self.slave,preexec_fn=session,env={**os.environ,'TERM':'xterm-256color','LANG':'C.UTF-8'})
    def pump(self,seconds=.1):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            if select.select([self.master],[],[],min(.05,max(0,deadline-time.monotonic())))[0]:
                try:data=os.read(self.master,65536)
                except OSError:break
                if not data:break
                self.raw+=data;self.screen.feed(self.decoder.decode(data))
    def wait(self,predicate,timeout=6):
        end=time.monotonic()+timeout
        while not predicate():
            self.pump()
            if time.monotonic()>end:raise AssertionError(('PTY timeout',self.process.poll(),self.screen.text(),self.raw[-1000:]))
    def text(self):self.pump();return self.screen.text()
    def key(self,key):os.write(self.master,key);self.pump(.12)
    def snapshot(self,name):self.snapshots[name]=self.text()
    def finish(self,expected=0):
        self.wait(lambda:self.process.poll() is not None);self.pump()
        assert self.process.returncode==expected,(self.process.returncode,self.raw[-1000:])
        assert termios.tcgetattr(self.slave)==self.original,'raw-mode flags were not restored'
        assert not self.screen.alternate,'alternate screen was not restored'
        assert {x['id'] for x in links()}=={x['id'] for x in self.before}
        assert signatures(attached())==signatures(self.baseline)
        return {'returncode':self.process.returncode,'raw_restored':True,'alternate_restored':True,'links_detached':True,'snapshots':self.snapshots}
    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:self.process.wait(timeout=4)
            except subprocess.TimeoutExpired:self.process.kill();self.process.wait(timeout=4)
        os.close(self.master);os.close(self.slave)
    def resize(self,cols,rows):
        self.screen.reset(cols,rows);fcntl.ioctl(self.slave,termios.TIOCSWINSZ,struct.pack('HHHH',rows,cols,0,0));self.process.send_signal(signal.SIGWINCH);self.pump(.25)

def normal(binary,obj,mode):
    t=Terminal([binary,'connect','--object',obj,'--tui','--interval-ms','100']+(['--enforce'] if mode=='enforce' else []));s=Server(socket.AF_INET,'127.0.0.1')
    try:
        t.wait(lambda:'準備完了' in t.text());t.snapshot('empty')
        t.key(b'x');assert t.process.poll() is None,'ordinary input exited'
        c=client(s.family,s.address,s.port);assert c['errno']==0
        t.wait(lambda:f'127.0.0.1:{s.port}' in t.text());t.snapshot('allowed')
        t.key(b'b')
        if mode=='enforce':
            t.wait(lambda:'確認:' in t.text());assert 'rules=0' in control(binary,'list')
            t.key(b'\x1b');t.wait(lambda:'取り消しました' in t.text());assert 'rules=0' in control(binary,'list')
            t.key(b'b');t.key(b'\r');t.wait(lambda:'成功:' in t.text());assert 'rules=1' in control(binary,'list')
            assert client(s.family,s.address,s.port)['errno']==errno.EPERM
            t.wait(lambda:'拒否 / DENY' in t.text());t.snapshot('denied')
            # Duplicate update must be visible and cannot alter the current map.
            t.key(b'b');t.key(b'\r');t.wait(lambda:'更新失敗:' in t.text());assert 'rules=1' in control(binary,'list');t.snapshot('duplicate_error')
            t.key(b'\t');t.key(b'd');t.wait(lambda:'解除する' in t.text());t.key(b'\r');t.wait(lambda:'成功: 解除' in t.text())
            assert 'rules=0' in control(binary,'list') and client(s.family,s.address,s.port)['errno']==0
            # Fill the real map, then attempt the selected destination from the UI.
            for port in range(10000,10016):control(binary,'add','127.0.0.1',port)
            t.key(b'\t');t.key(b'b');t.key(b'\r');t.wait(lambda:'更新失敗:' in t.text());assert 'rules=16' in control(binary,'list');t.snapshot('capacity_error')
        else:
            t.wait(lambda:'監視モード:' in t.text());t.key(b'\r');assert 'rules=0' in control(binary,'list');t.snapshot('observe_disabled')
        t.resize(70,24);t.wait(lambda:'拒否ルール' in t.text());t.snapshot('stacked_resize')
        t.resize(40,10);t.wait(lambda:'広げて' in t.text());t.snapshot('small_resize')
        t.resize(120,32);t.wait(lambda:'拒否ルール' in t.text())
        t.key(b'q' if mode=='enforce' else b'\x03');result=t.finish()
        assert client(s.family,s.address,s.port)['errno']==0
        result['mode']=mode;return result
    finally:t.close();s.close()

def exits(binary,obj,partial,fixture):
    results=[]
    for label,args,expected in [('term',[binary,'connect','--object',obj,'--tui'],0),('duration',[binary,'connect','--object',obj,'--tui','--duration-ms','200'],0),('missing_object',[binary,'connect','--object','/no/warden-object','--tui'],1),('partial_attach',[binary,'connect','--object',partial,'--tui'],1),('draw_error',[fixture,'--ignored','--exact','tui::runtime::tests::draw_error_restores_terminal_in_pty','--nocapture'],0)]:
        t=Terminal(args)
        try:
            if label=='term':t.wait(lambda:'準備完了' in t.text());t.process.send_signal(signal.SIGTERM)
            result=t.finish(expected)
            if label=='draw_error':assert b'1 passed' in t.raw and b'0 ignored' in t.raw, t.raw
            result['case']=label;results.append(result)
        finally:t.close()
    return results

def loss(binary,obj):
    t=Terminal([binary,'connect','--object',obj,'--tui','--interval-ms','100','--reader-delay-ms','800'])
    closed=socket.socket();closed.bind(('127.0.0.1',0));port=closed.getsockname()[1];closed.close()
    try:
        t.wait(lambda:'準備完了' in t.text())
        c=burst_client(socket.AF_INET,'127.0.0.1',port,'refused',count=4096,linger=0)
        assert c['refused']==4096
        def lost():
            match=re.search(r'ring=(\d+)',t.text())
            return match and int(match[1])>0
        t.wait(lost,timeout=8);t.snapshot('loss');match=re.search(r'ring=(\d+)',t.text());dropped=int(match[1])
        t.key(b'q');result=t.finish();result['ring_dropped']=dropped;return result
    finally:t.close()

def killed(binary,obj):
    t=Terminal([binary,'connect','--object',obj,'--tui','--enforce']);s=Server(socket.AF_INET,'127.0.0.1')
    try:
        t.wait(lambda:'準備完了' in t.text());control(binary,'add',s.address,s.port)
        assert client(s.family,s.address,s.port)['errno']==errno.EPERM
        t.process.kill();t.process.wait(timeout=5);t.pump()
        assert termios.tcgetattr(t.slave)!=t.original and t.screen.alternate
        assert {x['id'] for x in links()}=={x['id'] for x in t.before}
        assert signatures(attached())==signatures(t.baseline)
        assert client(s.family,s.address,s.port)['errno']==0
        termios.tcsetattr(t.slave,termios.TCSANOW,t.original)
        os.write(t.slave,b'\x1b[?1049l\x1b[?25h');t.pump()
        assert termios.tcgetattr(t.slave)==t.original and not t.screen.alternate
        return {'links_detached':True,'connection_restored':True,'automatic_terminal_restore':False,'manual_terminal_restore':True}
    finally:t.close();s.close()

def main():
    run('systemctl','start','warden-test.slice');binary,obj,partial,fixture=sys.argv[1:]
    non_tty=subprocess.run([binary,'connect','--object',obj,'--tui'],capture_output=True,text=True,timeout=5)
    assert non_tty.returncode!=0 and 'interactive stdin/stdout' in non_tty.stderr
    print(json.dumps({'kernel':run('uname','-r'),'non_tty_rejected':True,'keyboard':[normal(binary,obj,mode) for mode in ['observe','enforce']],'exit_and_errors':exits(binary,obj,partial,fixture),'loss':loss(binary,obj),'sigkill':killed(binary,obj),'ssh_active':run('systemctl','is-active','sshd.service')=='active'},ensure_ascii=False,indent=2))
if __name__=='__main__':main()

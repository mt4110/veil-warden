#!/usr/bin/env python3
"""Five-stage M3 demo using only synthetic loopback traffic."""
# SPDX-License-Identifier: MIT
import errno
import json
import socket
import sys
from connect_monitor import Monitor, run
from connect_policy import Server, client, control

def main():
    binary,obj=sys.argv[1:]
    run('systemctl','start','warden-test.slice')
    server=Server(socket.AF_INET,'127.0.0.1');monitor=None;stages=[]
    def attempt(label,expected):
        c=client(server.family,server.address,server.port)
        assert c['errno']==expected,c
        stages.append({'stage':label,'destination':f'{server.address}:{server.port}','client_errno':c['errno'],'receiver_messages':len(server.received)})
    try:
        monitor=Monitor(binary,obj)
        attempt('1 observe',0);monitor.stop('term')
        monitor=Monitor(binary,obj,extra=['--enforce'])
        control(binary,'add',server.address,server.port)
        before=len(server.received);attempt('2 deny',errno.EPERM);assert len(server.received)==before
        control(binary,'remove',server.address,server.port);attempt('3 remove',0)
        control(binary,'add',server.address,server.port)
        before=len(server.received);attempt('4 deny again',errno.EPERM);assert len(server.received)==before
        monitor.stop('term');attempt('5 exit restores',0)
        print(json.dumps({'stages':stages,'links_detached':True,'ssh_active':run('systemctl','is-active','sshd.service')=='active'},indent=2))
    finally:
        if monitor:monitor.cleanup()
        server.close()
if __name__=='__main__':main()

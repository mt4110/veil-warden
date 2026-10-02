#!/usr/bin/env python3
"""M0 feature and scope evidence. No custom network BPF program is attached."""
# SPDX-License-Identifier: MIT
import json
import os
from pathlib import Path
import socket
import subprocess
import threading

SCOPE = "/warden.slice/warden-test.slice"


def run(*args):
    return subprocess.check_output(args, text=True, timeout=60).strip()


def main():
    if os.geteuid() != 0:
        raise SystemExit("Run sudo warden-preflight inside the dedicated VM")
    if run("uname", "-m") != "aarch64":
        raise SystemExit("Expected the ARM64 sandbox")
    if run("findmnt", "-n", "-o", "FSTYPE", "/sys/fs/cgroup") != "cgroup2":
        raise SystemExit("cgroup v2 is required")
    if not Path("/sys/kernel/btf/vmlinux").is_file():
        raise SystemExit("Kernel BTF is unavailable")
    features = json.loads(run("bpftool", "-j", "feature", "probe", "kernel"))
    required = {
        "program_types": ["have_cgroup_skb_prog_type", "have_cgroup_sock_addr_prog_type"],
        "map_types": ["have_ringbuf_map_type", "have_hash_map_type", "have_percpu_array_map_type"],
    }
    for group, names in required.items():
        for name in names:
            if features.get(group, {}).get(name) is not True:
                raise SystemExit(f"Required BPF feature is unavailable: {name}")
    links = json.loads(run("bpftool", "-j", "link", "show"))
    ssh_scope = run("systemctl", "show", "sshd.service", "--property=ControlGroup", "--value")
    if not ssh_scope or ssh_scope.startswith(SCOPE):
        raise SystemExit("Management SSH must be outside the experiment scope")
    servers = []
    threads = []
    received = []
    failures = []

    def accept(server, family):
        try:
            with server.accept()[0] as connection:
                if connection.recv(64) != b"warden-m0-synthetic":
                    raise ValueError("unexpected synthetic input")
                received.append(family)
        except Exception as error:
            failures.append(str(error))

    try:
        for family, address in [(socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")]:
            server = socket.socket(family, socket.SOCK_STREAM)
            server.settimeout(15)
            server.bind((address, 0))
            server.listen(1)
            servers.append(server)
            thread = threading.Thread(target=accept, args=(server, family))
            thread.start()
            threads.append(thread)
        client = r'''
import json, socket, sys
scope = open('/proc/self/cgroup').read().strip().split(':', 2)[2]
if not scope.startswith('/warden.slice/warden-test.slice/'):
    raise SystemExit('Client is outside the test slice')
# Each socket is created after systemd has placed this process in the slice.
for family, address, port in [(socket.AF_INET, '127.0.0.1', int(sys.argv[1])),
                              (socket.AF_INET6, '::1', int(sys.argv[2]))]:
    with socket.socket(family, socket.SOCK_STREAM) as connection:
        connection.settimeout(5)
        connection.connect((address, port))
        connection.sendall(b'warden-m0-synthetic')
print(json.dumps({'client_scope': scope, 'socket_created_after_scope': True}))
'''
        output = run("systemd-run", "--quiet", "--wait", "--pipe", "--collect",
                     "--unit=warden-m0-probe", "--slice=warden-test.slice",
                     "python3", "-c", client, *(str(s.getsockname()[1]) for s in servers))
        for thread in threads:
            thread.join(16)
        if failures or sorted(received) != [socket.AF_INET, socket.AF_INET6]:
            raise SystemExit(f"Synthetic loopback check failed: {failures}")
        evidence = {
            "kernel": run("uname", "-r"),
            "architecture": run("uname", "-m"),
            "nixos": run("nixos-version"),
            "cgroup": "v2", "btf": True,
            "required_bpf_features": required,
            "existing_bpf_link_count": len(links),
            "cgroup_link_create": "deferred to M1 actual attach test",
            "ssh_scope": ssh_scope,
            "client": json.loads(output),
            "loopback_ipv4_received": True,
            "loopback_ipv6_received": True,
            "custom_bpf_attached": False,
        }
        print(json.dumps(evidence, indent=2))
    finally:
        for server in servers:
            server.close()


if __name__ == "__main__":
    main()

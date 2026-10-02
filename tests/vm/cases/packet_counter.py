#!/usr/bin/env python3
"""M1 real-kernel acceptance; synthetic loopback only, no payload inspection."""
# SPDX-License-Identifier: MIT
import json
import os
from pathlib import Path
import queue
import re
import signal
import socket
import subprocess
import sys
import threading
import time

SCOPE = "/sys/fs/cgroup/warden.slice/warden-test.slice"


def run(*args):
    return subprocess.check_output(args, text=True, timeout=15).strip()


def links():
    return {v["id"] for v in json.loads(run("bpftool", "-j", "link", "show"))}


def attached():
    return json.loads(run("bpftool", "-j", "cgroup", "show", SCOPE))


def traffic(family, inside, ordinal):
    address = "127.0.0.1" if family == socket.AF_INET else "::1"
    with socket.socket(family, socket.SOCK_DGRAM) as receiver:
        receiver.settimeout(5)
        receiver.bind((address, 0))
        source = r'''
import socket, sys
scope = open('/proc/self/cgroup').read().strip().split(':', 2)[2]
inside = sys.argv[1] == 'inside'
assert scope.startswith('/warden.slice/warden-test.slice/') == inside, scope
with socket.socket(int(sys.argv[2]), socket.SOCK_DGRAM) as sender:
    for _ in range(8): sender.sendto(b'warden-m1-synthetic', (sys.argv[3], int(sys.argv[4])))
print(scope)
'''
        scope = run("systemd-run", "--quiet", "--wait", "--pipe", "--collect",
                    f"--unit=warden-m1-client-{ordinal}",
                    "--slice=warden-test.slice" if inside else "--slice=system.slice",
                    "python3", "-c", source, "inside" if inside else "outside",
                    str(family), address, str(receiver.getsockname()[1]))
        for _ in range(8):
            assert receiver.recv(128) == b"warden-m1-synthetic"
        return scope


def check(binary, obj, exit_mode, ordinal):
    before = links()
    baseline_attachments = attached()
    assert not any(v["name"] == "count_egress" for v in baseline_attachments)
    command = [binary, "--object", obj, "--interval-ms", "20"]
    if exit_mode == "normal": command += ["--samples", "250"]
    process = subprocess.Popen(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    lines = queue.Queue()
    transcript = []

    def read():
        for line in process.stdout:
            transcript.append(line.strip())
            lines.put(line.strip())
    reader = threading.Thread(target=read, daemon=True)
    reader.start()

    def count():
        deadline = time.monotonic() + 3
        while time.monotonic() < deadline:
            try: line = lines.get(timeout=0.2)
            except queue.Empty:
                if process.poll() is not None: raise AssertionError(process.stderr.read())
                continue
            match = re.fullmatch(r"packets=(\d+) delta=\d+", line)
            if match:
                value = int(match[1])
                while True:
                    try: more = lines.get_nowait()
                    except queue.Empty: return value
                    match = re.fullmatch(r"packets=(\d+) delta=\d+", more)
                    if match: value = int(match[1])
        raise AssertionError("counter output timed out")

    try:
        initial = count()
        live = links()
        new = live - before
        assert len(new) == 1, (before, live)
        attachment = attached()
        assert len(attachment) == len(baseline_attachments) + 1, attachment
        assert all(v in attachment for v in baseline_attachments), attachment
        details = [v for v in json.loads(run("bpftool", "-j", "link", "show")) if v["id"] in new]
        assert details[0]["type"] == "cgroup", details
        failures = []
        for extra in [["--object", obj, "--cgroup", "/"], ["--object", "/missing-warden-object"], ["--object", obj, "--samples", "1"]]:
            failed = subprocess.run([binary, *extra], capture_output=True, text=True, timeout=5)
            assert failed.returncode != 0, extra
            assert links() == live, "failed startup changed live links"
            failures.append({"arguments": extra, "rejected": True})
        deltas = {}
        current = initial
        for family in [socket.AF_INET, socket.AF_INET6]:
            baseline = count()
            scope = traffic(family, True, f"{ordinal}-{family}-in")
            time.sleep(0.1)
            current = count()
            assert current > baseline, (family, baseline, current)
            deltas[str(family)] = {"increase": current - baseline, "client_scope": scope}
        outside_before = count()
        for family in [socket.AF_INET, socket.AF_INET6]:
            traffic(family, False, f"{ordinal}-{family}-out")
        time.sleep(0.15)
        outside_after = count()
        assert outside_before == outside_after, (outside_before, outside_after)
        if exit_mode == "term": process.send_signal(signal.SIGTERM)
        elif exit_mode == "kill": process.send_signal(signal.SIGKILL)
        code = process.wait(timeout=8)
        reader.join(timeout=2)
        assert code == (-signal.SIGKILL if exit_mode == "kill" else 0), (code, process.stderr.read())
        assert ("detached" in transcript) == (exit_mode != "kill")
        deadline = time.monotonic() + 3
        while links() != before and time.monotonic() < deadline: time.sleep(0.05)
        assert links() == before, "BPF link remains after exit"
        final_attachments = attached()
        # systemd rebuilds its firewall programs when transient units come/go.
        # Compare every attachment field except the kernel-assigned program ID.
        def signatures(values):
            return sorted(json.dumps({k: v for k, v in item.items() if k != "id"}, sort_keys=True) for item in values)
        assert signatures(final_attachments) == signatures(baseline_attachments), "cgroup attachment roles changed after exit"
        assert run("systemctl", "is-active", "sshd.service") == "active"
        return {"exit": exit_mode, "returncode": code, "initial": initial,
                "families": deltas, "outside_before": outside_before,
                "outside_after": outside_after, "startup_failures": failures,
                "fd_link": details, "baseline_attachments": baseline_attachments,
                "final_attachments": final_attachments, "link_detached": True, "ssh_active": True}
    finally:
        if process.poll() is None:
            process.terminate()
            process.wait(timeout=5)
        process.stdout.close()
        process.stderr.close()


def main():
    assert os.geteuid() == 0, "run inside the VM with sudo"
    assert Path("/etc/hostname").read_text().strip() == "veil-warden-sandbox"
    run("systemctl", "start", "warden-test.slice")
    binary, obj = sys.argv[1:]
    evidence = {"kernel": run("uname", "-r"), "architecture": run("uname", "-m"),
                "scope": SCOPE, "cases": [check(binary, obj, mode, i)
                for i, mode in enumerate(["normal", "term", "kill"])]}
    print(json.dumps(evidence, indent=2))


if __name__ == "__main__":
    main()

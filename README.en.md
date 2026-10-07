# veil-warden

[日本語](README.md)

An experimental Rust and Aya project for observing network activity in a dedicated Linux VM and denying new connections using preconfigured rules. Only synthetic clients in the dedicated test cgroup are in scope.

**M0 through M5 are implemented and tested in the dedicated NixOS VM.** M1 counts egress SKBs. M2 reports TCP IPv4/IPv6 connection attempts, destinations, TGID/TID, and best-effort process names. M3 denies new TCP connections matching explicit destination rules in the dedicated cgroup. Observe is the default. Arguments are not read by default. M5 can evaluate one explicitly selected synthetic process once; payloads and environment variables remain out of scope.

## What is it for?

Explore how a selected Linux process group attempts new TCP connections and how explicit destination rules deny or restore future attempts. Start with the local Mac tutorial, then use the dedicated VM for real eBPF behavior. Mac application monitoring and cloud-wide protection are not implemented.

Use Wireshark for packet contents and protocol analysis. This project explores decisions at the connection hook. See [use cases and the TUI walkthrough](docs/USE_CASES.md) (Japanese) for alternatives and scope.

## Find the right document

| Goal | Start here |
| --- | --- |
| Understand use cases and try the sample screen | [Use cases and dry-run TUI](docs/USE_CASES.md) |
| Run, stop, or recover | [Operation guide](docs/GUIDE.md) / [short demo](docs/DEMO.md) |
| Understand design and safety | [Architecture](docs/ARCHITECTURE.md) / [safety boundaries](docs/SAFETY.md) |
| Check limitations and open work | [Known issues](docs/KNOWN_ISSUES.md) / [research themes](docs/RESEARCH.md) |
| Ask a question or contribute | [Support](SUPPORT.md) / [contributing](CONTRIBUTING.md) |
| Browse all documents and evidence | **[Documentation index](docs/README.md)** |

Cloudflare registration, organization membership, and deployment are not required. The current workflow uses Nix and local Linux VMs; dependency downloads require internet access. Detailed documents are currently in Japanese.

## Quick start

On an Apple Silicon Mac with Nix installed, run this once from the checkout:

```sh
./scripts/install-cli.sh
```

Open a new terminal, then open the **sample TUI** from any directory:

```sh
veil-warden
```

The installer builds a native Rust entry point at `~/.local/bin/veil-warden` and adds PATH to `.zshrc` (under `ZDOTDIR` when configured). The installer also builds the native sample renderer. With no command, the CLI opens a dry-run tutorial with three fictional events, no VM startup, and no real traffic or denial. Use b, Enter, n to simulate denial, then Tab, d, Enter, n to remove it. The screen is in Japanese.

Run `veil-warden tui` for real monitoring inside the Linux VM. That command starts the VMs, waits for authenticated SSH, builds missing artifacts, runs preflight, and opens the live TUI. The history remains empty until a process in the dedicated cgroup attempts a new TCP connection. Initial downloads can be several GiB. Existing artifacts are reused; after source changes, rerun `./scripts/install-cli.sh` for the Mac launcher and preview, then stop the sandbox and run `veil-warden build` for the VM runtime.

VMs run in the background under macOS launchd and survive terminal closure. They do not start automatically at login. Monitoring still runs inside the dedicated Linux VM and ends when the TUI exits; this does not implement M6 continuous monitoring or persistent rules.

Use `veil-warden tui --dry-run` for the sample screen. For the VM, use `veil-warden start`, `veil-warden tui --enforce`, `veil-warden demo`, `veil-warden status`, and `veil-warden stop`. Stop only affects VMs managed by this CLI; existing manually started VMs are reused and preserved. Stop those in their original terminal. Each VM has a 4 GiB memory limit. Logs live in `host-cli/builder.log` and `host-cli/sandbox.log` under the existing state directory. Reinstall if the checkout moves. The installer backs up an existing CLI, and does not delete logs or disks.

Show all help with `veil-warden -h`. For command-specific help, use `veil-warden tui -h` or `veil-warden build --help`. Choose Japanese or English with `--lang ja` or `--lang en`; when omitted, the locale selects Japanese for Japanese locales and English otherwise. Example: `veil-warden --lang ja -h`.

## Server deployment and updates

[Deployment design candidates](docs/DEPLOYMENT.md) (Japanese) describe systemd installation, possible DaemonSet placement, link/map lifetime, and update rollback. These are unimplemented and unverified proposals, with no zero-downtime, lossless logging, or production protection guarantee. Implementation awaits a concrete service requirement that existing tools cannot adequately meet.

## Manual VM setup (for development)

Use Nix on an Apple Silicon Mac. mise is not required. The first download needs internet access and several GiB of disk space.

Start the builder in terminal 1 and leave it running. In terminal 2, build and start the sandbox; that command also runs the preflight check.

```sh
# 1. Terminal 1
nix develop "path:$PWD" -c ./scripts/bootstrap-builder.sh
```

```sh
# 2. Terminal 2
nix develop "path:$PWD" -c ./scripts/start-vm.sh
```

The sandbox keeps running in the background after preflight. Its output is saved under `$XDG_CACHE_HOME/veil-warden-m0/sandbox/quickstart-vm.log`, or `~/.cache/veil-warden-m0/sandbox/quickstart-vm.log` if `XDG_CACHE_HOME` is unset. Stop it with `./scripts/ssh-vm.sh 'sudo poweroff'`. The scripts do not change host Nix privileges or system configuration. Each VM has a 4 GiB memory limit. The sandbox root is ephemeral; booting again restores the declared configuration without deleting a disk.

## M1 counter

With both local VMs running:

```sh
./scripts/build-counter-vm.sh
./scripts/test-counter-vm.sh
```

The counter always permits traffic. It collects no payloads or PIDs. It counts egress SKBs, which can differ from physical wire frames. Aya 0.14.0, aya-ebpf 0.2.1, nightly-2025-12-01, and bpf-linker 0.9.15 are pinned through Cargo and Nix.

## M2 connection monitor

With both local VMs running, use the shared build script:

```sh
./scripts/build-counter-vm.sh
./scripts/test-connect-vm.sh
```

The CLI's `connect` mode reports attempts, including refused connections. It does not determine connection success or safety. Kernel ring-buffer losses, decode errors, and bounded output-queue losses are counted separately. See [M2 evidence](milestones/02-connect-monitor/README.md).

## Documentation

The detailed guides are currently in Japanese:

- [Execution guide](docs/GUIDE.md)
- [Development guidelines](docs/GUIDELINES.md)
- [Safety rationale and limitations](docs/SAFETY.md)
- [Security policy](SECURITY.md)
- [MIT license](LICENSE.md)
- [Contributing](CONTRIBUTING.md)
- [Roadmap](docs/ROADMAP.md), [architecture](docs/ARCHITECTURE.md), and [M0 evidence](milestones/00-sandbox/README.md)

Architecture guards verify repository boundaries and the M0 policy. They do not establish runtime enforcement correctness. VM verification is separate, and no first-send secret-leak prevention is claimed.

## M3 connection policy

```sh
./scripts/build-counter-vm.sh
./scripts/test-policy-vm.sh
```

Inside the dedicated VM, run `connect --enforce --deny IP PORT` with the object path to install a rule before attaching. As root, use `policy add IP PORT`, `policy remove IP PORT`, and `policy list` while it runs. Rules match address family, address, port, and TCP; IPv4-mapped IPv6 is normalized to IPv4. Capacity is 16 rules. Updates are not persistent and errors return a nonzero exit status with the current map state.

VM acceptance covers IPv4/IPv6 denial and recovery, unaffected destinations and outside traffic, existing connections, capacity failure, partial attachment failure, and normal/SIGTERM/SIGKILL exits. All 4,096 attempts in the overload test were denied even with ring-buffer losses. This does not stop existing connections, inspect TLS, or prevent secret leaks. The monitor stops protecting new connections when it exits. See the [Japanese operation guide](docs/GUIDE.md#m3-接続拒否と解除の実行) and [M3 evidence](milestones/03-connect-policy/README.md).

## M3A demo and M4 TUI

```sh
./scripts/demo-policy-vm.sh
./scripts/tui-vm.sh             # observe
./scripts/tui-vm.sh --enforce   # explicit deny/remove actions
```

Run from an interactive terminal with both VMs started and artifacts built. The demo verifies observe, deny, remove, deny again, and recovery after exit. The TUI displays connection attempts, rules, mode, scope, and separate loss counters. History is bounded to 128 entries. Tab selects a pane, arrows or j/k select a row, b prepares a deny rule, d prepares removal, Enter confirms the displayed tuple, Esc cancels, and q/Ctrl+C exits. Observe mode rejects policy changes. Normal exits restore the terminal and release the owned BPF links; SIGKILL cannot restore terminal settings. See the [operation guide](docs/GUIDE.md#m4-tuiの実行) and [PTY evidence](milestones/04-tui/README.md).

## M5 secret warnings

`connect --scan-argv PID:START_TICKS` evaluates only the explicitly selected test process in the dedicated cgroup. CLI and TUI expose evaluation state, fixed rule IDs and counts, never argument values. Detection is not evidence of transmission and never adds deny rules. Unevaluated results remain distinct from no matches. Limits: 16 KiB, one process, a two-second parent wait, 256 MiB virtual address space and two CPU seconds for the child. See the [specification and limitations](docs/SECRET_WARNING.md) and [M5 evidence](milestones/05-secret-warning/README.md). This research ends at M5; M6 service work is deferred.

## OSS information

See the [changelog](CHANGELOG.md), [security policy](SECURITY.md), [license](LICENSE.md), and [third-party attribution](docs/third-party/veil-rs-MIT.txt). The [documentation index](docs/README.md) links every guide and milestone. Research candidates do not authorize implementation.

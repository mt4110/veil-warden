# veil-warden

[日本語](README.md)

An experimental Rust and Aya project for observing network activity in a dedicated Linux VM and denying new connections using preconfigured rules. Only synthetic clients in the dedicated test cgroup are in scope.

**M0 through M3 are complete and verified in the dedicated NixOS VM.** M1 counts egress SKBs. M2 reports TCP IPv4/IPv6 connection attempts, destinations, TGID/TID, and best-effort process names. M3 denies new TCP connections matching explicit destination rules in the dedicated cgroup. Observe is the default. No payloads, command arguments, or environment variables are collected.

## Getting started

Use Nix. mise is not required. The NixOS environment, kernel, Rust tools, Markdown linter, and QEMU come from the nixpkgs revision and content hash in `flake.lock`.

```sh
nix develop "path:$PWD"
./scripts/check.sh
```

On an Apple Silicon Mac, start the bootstrap builder in one terminal:

```sh
./scripts/bootstrap-builder.sh
```

In a second terminal, build and start the sandbox:

```sh
./scripts/build-vm.sh
./scripts/run-vm.sh
```

Once the sandbox boots, use a third terminal:

```sh
./scripts/ssh-vm.sh 'sudo warden-preflight'
```

The scripts do not change host Nix privileges or system configuration. Each VM has a 4 GiB memory limit. Initial setup requires several GiB of disk space. The sandbox root is ephemeral; booting again restores the declared configuration without deleting a disk.

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

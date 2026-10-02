# veil-warden

[日本語](README.md)

An experimental Rust and Aya project for observing network activity in a dedicated Linux VM and denying new connections using preconfigured rules. Only synthetic clients in the dedicated test cgroup are in scope.

**M0 sandbox setup is complete**, including VM feature probes, synthetic IPv4/IPv6 traffic, and recovery verification. The monitoring and enforcement engine has not been implemented. M3 is the planned weekend completion point.

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
./scripts/ssh-vm.sh 'sudo poweroff'
```

The scripts do not change host Nix privileges or system configuration. Each VM has a 4 GiB memory limit. Initial setup requires several GiB of disk space. The sandbox root is ephemeral; booting again restores the declared configuration without deleting a disk.

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

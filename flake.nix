{
  description = "veil-warden reproducible M0 sandbox and checks";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/4feb8eb8bf30f323a8a5d285f14ee51d6a7197b1";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      eachSystem = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
      sandbox = builtins.fromTOML (builtins.readFile ./config/sandbox.toml);
      mkVm =
        hostSystem:
        nixpkgs.lib.nixosSystem {
          system = "aarch64-linux";
          specialArgs = { inherit sandbox; };
          modules = [
            ./infra/nixos/vm.nix
            { virtualisation.host.pkgs = nixpkgs.legacyPackages.${hostSystem}; }
          ];
        };
      vm = mkVm "aarch64-darwin";
      linuxVm = mkVm "aarch64-linux";
      guestPkgs = nixpkgs.legacyPackages.aarch64-linux;
      registration = guestPkgs.closureInfo {
        rootPaths = [ linuxVm.config.system.build.toplevel ];
      };
      closure = guestPkgs.closureInfo {
        rootPaths = [
          linuxVm.config.system.build.toplevel
          registration
        ];
      };
      bundle =
        guestPkgs.runCommand "veil-warden-sandbox-bundle"
          {
            nativeBuildInputs = [
              guestPkgs.gnutar
              guestPkgs.erofs-utils
            ];
          }
          ''
            mkdir -p "$out"
            cp -L ${linuxVm.config.system.build.toplevel}/kernel "$out/kernel"
            cp -L ${linuxVm.config.system.build.initialRamdisk}/initrd "$out/initrd"
            tar --create --absolute-names --verbatim-files-from \
              --transform 'flags=rSh;s|/nix/store/||' --files-from ${closure}/store-paths \
              | mkfs.erofs --quiet --force-uid=0 --force-gid=0 -L nix-store \
                -U eb176051-bd15-49b7-9e6b-462e0b467019 -T 0 --hard-dereference \
                --tar=f "$out/store.img"
            echo 'root=fstab init=${linuxVm.config.system.build.toplevel}/init regInfo=${registration}/registration console=ttyAMA0,115200n8 ${builtins.concatStringsSep " " linuxVm.config.boot.kernelParams}' > "$out/kernel-params"
          '';

      cleanSource = nixpkgs.lib.cleanSourceWith {
        src = self;
        filter =
          path: type:
          let
            base = builtins.baseNameOf path;
          in
          !(builtins.elem base [
            ".git"
            "target"
            "artifacts"
            "result"
          ]);
      };
    in
    {
      nixosConfigurations.sandbox = vm;
      packages = eachSystem (pkgs: {
        sandbox-vm = (mkVm pkgs.stdenv.hostPlatform.system).config.system.build.vm;
        sandbox-bundle = bundle;
        default = pkgs.rustPlatform.buildRustPackage {
          pname = "veil-warden-architecture-guard";
          version = "0.0.0";
          src = cleanSource;
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = true;
        };
      });
      devShells = eachSystem (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            rustc
            cargo
            rustfmt
            clippy
            markdownlint-cli2
            nixfmt
            python3
            openssh
            jq
            git
            nix
            shellcheck
          ];
          shellHook = ''
            export RUST_BACKTRACE=1
            echo "veil-warden M0: Rust ${pkgs.rustc.version}; no eBPF attach"
          '';
        };
      });
      checks = eachSystem (pkgs: {
        architecture = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
        markdown =
          pkgs.runCommand "veil-warden-markdown"
            {
              nativeBuildInputs = [ pkgs.markdownlint-cli2 ];
            }
            ''
              cd ${cleanSource}
              markdownlint-cli2
              touch "$out"
            '';
        sandbox-policy =
          assert builtins.all (a: a.assertion) vm.config.assertions;
          pkgs.writeText "veil-warden-sandbox-policy" (builtins.toJSON self.sandboxFacts);
      });
      sandboxFacts = {
        nixpkgsRevision = nixpkgs.rev;
        nixosVersion = vm.config.system.nixos.release;
        kernelVersion = vm.config.boot.kernelPackages.kernel.version;
        rustVersion = nixpkgs.legacyPackages.aarch64-darwin.rustc.version;
        nixVersion = nixpkgs.legacyPackages.aarch64-darwin.nix.version;
        bpfToolsVersion = guestPkgs.bpftools.version;
        pythonVersion = guestPkgs.python3.version;
        markdownlintVersion = nixpkgs.legacyPackages.aarch64-darwin.markdownlint-cli2.version;
        qemuVersion = vm.config.virtualisation.qemu.package.version;
        inherit (sandbox.m0) mode attach_enabled scope;
        inherit (vm.config.virtualisation)
          cores
          memorySize
          sharedDirectories
          ;
        ephemeralRoot = vm.config.virtualisation.diskImage == null;
        bootstrap = sandbox.bootstrap;
        ssh = {
          bind = sandbox.m0.ssh_bind;
          port = sandbox.m0.ssh_port;
          permitRootLogin = vm.config.services.openssh.settings.PermitRootLogin;
          passwordAuthentication = vm.config.services.openssh.settings.PasswordAuthentication;
        };
      };
    };
}

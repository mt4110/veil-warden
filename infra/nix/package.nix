# SPDX-License-Identifier: MIT
{
  pkgs,
  src,
  bpfToolchain,
}:
let
  lib = pkgs.lib;
  veilSource = pkgs.fetchgit {
    url = "https://github.com/mt4110/veil-rs.git";
    rev = "83592f5cfb73059f3eaadefcb98bb38262f2ff78";
    hash = "sha256-Kcdik2+N8Tqw2j3yhrMcuE1CrAqutu9pmmXMn9HGRmI=";
  };
  lock = {
    lockFile = ../../Cargo.lock;
    outputHashes."veil-config-0.17.0" = "sha256-Kcdik2+N8Tqw2j3yhrMcuE1CrAqutu9pmmXMn9HGRmI=";
  };
  workspaceVendor = pkgs.rustPlatform.importCargoLock lock;
  # Snapshot from the pinned 2025-12-01 rust-src component. Checked against
  # the actual toolchain below, avoiding import-from-derivation at evaluation.
  stdVendor = pkgs.rustPlatform.importCargoLock {
    lockFile = ../../config/bpf-stdlib-Cargo.lock;
  };
  bpfVendor = pkgs.runCommand "cargo-vendor-dir" { } ''
    mkdir -p "$out"
    cp -rs ${workspaceVendor}/. "$out/"
    for crate in ${stdVendor}/*; do
      name=$(basename "$crate")
      if [ "$name" != Cargo.lock ] && [ ! -e "$out/$name" ]; then
        ln -s "$crate" "$out/$name"
      fi
    done
  '';
  cli = pkgs.rustPlatform.buildRustPackage {
    pname = "veil-warden-cli";
    version = "0.1.0";
    inherit src;
    cargoLock = lock;
    nativeBuildInputs = [ pkgs.pkg-config ];
    buildInputs = [ pkgs.openssl ];
    # importCargoLock vendors workspace crates separately. veil-core embeds
    # assets from a sibling workspace directory; retain those exact pinned
    # assets rather than silently compiling an empty/default ruleset.
    postPatch = ''
      test -f ${veilSource}/crates/veil/rules/default/00_manifest.toml
      substituteInPlace "$NIX_BUILD_TOP/cargo-vendor-dir/veil-core-0.17.0/src/rules/builtin.rs" \
        --replace-fail '"../veil/rules/default/"' '"${veilSource}/crates/veil/rules/default/"' \
        --replace-fail '"../../../veil/rules/default/00_manifest.toml"' '"${veilSource}/crates/veil/rules/default/00_manifest.toml"'
    '';
    cargoBuildFlags = [
      "-p"
      "veil-warden"
      "--bin"
      "veil-warden"
      "--locked"
    ];
    cargoTestFlags = [
      "-p"
      "veil-warden"
      "--locked"
    ];
    meta.mainProgram = "veil-warden";
  };
  bpf = pkgs.stdenv.mkDerivation {
    pname = "veil-warden-ebpf";
    version = "0.1.0";
    inherit src;
    nativeBuildInputs = [
      bpfToolchain
      pkgs.bpf-linker
    ];
    dontConfigure = true;
    dontStrip = true;
    dontPatchELF = true;
    buildPhase = ''
      runHook preBuild
      cmp ${../../config/bpf-stdlib-Cargo.lock} \
        ${bpfToolchain}/lib/rustlib/src/rust/library/Cargo.lock
      export CARGO_HOME="$TMPDIR/cargo-home"
      export CARGO_TARGET_DIR="$TMPDIR/target"
      mkdir -p "$CARGO_HOME"
      ln -s ${bpfVendor} cargo-vendor-dir
      cat ${bpfVendor}/.cargo/config.toml >> .cargo/config.toml
      cargo build --offline --locked \
        -p veil-warden-ebpf --bin veil-warden-ebpf --features ebpf \
        --target bpfel-unknown-none -Z build-std=core --release -j "$NIX_BUILD_CORES"
      runHook postBuild
    '';
    installPhase = ''
      mkdir -p "$out/lib/veil-warden"
      cp "$CARGO_TARGET_DIR/bpfel-unknown-none/release/veil-warden-ebpf" \
        "$out/lib/veil-warden/veil-warden-ebpf"
    '';
  };
  bundle =
    pkgs.runCommand "veil-warden-0.1.0"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        meta = {
          platforms = lib.platforms.linux;
          mainProgram = "veil-warden";
        };
      }
      ''
        mkdir -p "$out/bin" "$out/lib/veil-warden" "$out/share/veil-warden"
        cp ${cli}/bin/veil-warden "$out/bin/veil-warden"
        cp ${bpf}/lib/veil-warden/veil-warden-ebpf "$out/lib/veil-warden/"
        cp ${src}/LICENSE.md "$out/share/veil-warden/LICENSE.md"
        cp ${src}/docs/NIX_PREFLIGHT.md "$out/share/veil-warden/README.md"
        python3 - "$out" <<'PY'
        import hashlib, json, pathlib, re, sys
        root = pathlib.Path(sys.argv[1])
        abi = re.search(r'pub const ABI_VERSION: u16 = (\d+);',
            pathlib.Path('${src}/crates/veil-warden-common/src/lib.rs').read_text())
        if abi is None:
            raise SystemExit('Missing event ABI version')
        files = ['bin/veil-warden', 'lib/veil-warden/veil-warden-ebpf']
        manifest = {
            'schema': 'veil-warden-package-v1', 'system': '${pkgs.stdenv.hostPlatform.system}',
            'source_store_path': '${src}', 'event_abi': int(abi[1]),
            'bpf_target': 'bpfel-unknown-none', 'bpf_cpu': 'v3',
            'loader_scope': 'dedicated-sandbox-only', 'deployment_ready': False,
            'sha256': {p: hashlib.sha256((root / p).read_bytes()).hexdigest() for p in files},
        }
        (root / 'share/veil-warden/manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        PY
        python3 ${src}/scripts/verify-package.py "$out"
        # A package check, not an attach test. All diagnostics bypass the loader.
        "$out/bin/veil-warden" diagnose --help >/dev/null
      '';
in
{
  warden-cli = cli;
  warden-ebpf = bpf;
}
// lib.optionalAttrs pkgs.stdenv.isLinux { veil-warden = bundle; }

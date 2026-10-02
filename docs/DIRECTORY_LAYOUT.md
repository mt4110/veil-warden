# veil-warden ディレクトリ構成

## 作成済みの構成

環境設定、検証ツール、方針文書を置いています。M1〜M3の3クレートと観測・制御コードもworkspaceに追加しています。

```text
veil-warden/
├── README.md                       # 日本語の入口
├── README.en.md                    # 英語の入口
├── SECURITY.md                     # セキュリティ方針
├── LICENSE.md                      # MIT 本文と日本語補足
├── CONTRIBUTING.md                 # 貢献ガイド
├── Cargo.toml / Cargo.lock         # 検証ツールの workspace と依存固定
├── rust-toolchain.toml             # stable 1.95.0
├── flake.nix / flake.lock          # 単一の環境定義・nixpkgs 固定
├── .markdownlint-cli2.jsonc         # Markdown リント
├── .github/workflows/checks.yml    # 静的検証と再現ビルド
├── config/
│   ├── sandbox.toml                # M0 の対象・モード・資源
│   └── architecture.toml           # 許可する workspace 内の依存方向
├── infra/nixos/vm.nix              # NixOS VM、公開鍵認証、slice
├── scripts/
│   ├── build-counter.sh            # 固定BPF toolchain / stable CLI
│   ├── build-counter-vm.sh         # MacからLinux builderで構築
│   ├── test-counter-vm.sh          # M1実VM試験
│   ├── test-connect-vm.sh          # M2実VM試験
│   ├── test-policy-vm.sh           # M3実VM試験
│   ├── demo-policy-vm.sh           # M3Aの5段階デモ
│   ├── test-tui-vm.sh              # M4実PTY試験
│   ├── tui-vm.sh                   # 実端末からTUI起動
│   ├── bootstrap-builder.sh        # Mac 内のローカル Linux builder
│   ├── build-vm.sh                 # Linux 内で独立した VM イメージを構築
│   ├── run-vm.sh                   # ホストで専用 VM を起動
│   ├── ssh-vm.sh                   # 起動ごとのホスト鍵を検証
│   ├── validate-public-keys.py      # 秘密が store に入らない境界の検査
│   └── check.sh                    # 静的検証
├── tools/architecture-guard/
│   ├── Cargo.toml
│   ├── src/main.rs
│   └── tests/architecture.rs        # Rust による構造・方針の検査
├── tests/
│   ├── unit/test_public_keys.py     # 公開鍵と秘密・不正データの区別
│   └── vm/
│       ├── README.md
│       ├── preflight.py             # VM機能、cgroup、IPv4/IPv6の実測
│       └── cases/                   # packet_counter / connect_monitor / connect_policy
├── docs/                           # 設計、実行、安全性、開発ガイドライン
├── milestones/                     # M0〜M6 の作業・合格条件
└── artifacts/                      # ローカル検証結果、Git 対象外
```

環境の flake はルートに一つだけ置きます。NixOS のモジュールは infra/nixos に分け、別の flake/lock を作って版が食い違う構成を避けます。

VM の秘密鍵・ディスク・起動状態はリポジトリ外の `$HOME/.cache/veil-warden-m0` に保存します。詳細は [実行ガイド](GUIDE.md) を参照してください。

## ランタイム構成

M1のcounterに加え、M2のconnect、decode、events、process、outputを作成済みです。M3のルールMap・判断はkernelのconnect.rs、追加/解除/一覧はuserのpolicy.rsに実装しています。M4のTUIも実装済みで、scanは未作成です。M1は独立したビルドスクリプトを使い、build.rsは不要です。

```text
crates/
├── veil-warden-common/
│   ├── Cargo.toml
│   └── src/lib.rs                  # no_std、共有 ABI・Map キー
├── veil-warden-ebpf/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── counter.rs              # M1: cgroup_skb egress
│       ├── connect.rs              # M2/M3: connect4/connect6、事前ルール参照
│       └── partial_fixture.rs      # connect6欠落の起動失敗fixture
└── veil-warden/
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs / cli.rs
    │   ├── loader.rs                # Linux限定、Map/link 所有
    │   ├── decode.rs / events.rs / process.rs / output.rs
    │   ├── policy.rs                 # M3: tuple検証、Map操作、root制御socket
    │   ├── tui.rs                   # M4: 状態、描画、入力、端末復元
    │   └── scan.rs                  # M5予定
    └── tests/
```

役割が増えた段階で modules を分け、M1 から空ファイルを大量に置きません。xtask は現時点で不要です。M6 の service.nix も未作成です。

macOS は編集と純粋ロジック試験、VM 管理を担当します。BPF の実ロード・アタッチと通信制御の検証は Linux VM 内で行い、macOS の試験結果で代用しません。

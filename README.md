# veil-warden

[English](README.en.md)

Rust と Aya で Linux VM 内の通信を観測し、事前ルールで新規接続を拒否する実験プロジェクトです。指定した合成テストプロセスだけを対象にします。

**M0は完了しています。M1の専用cgroup送信パケットカウンタも完了し、IPv4/IPv6・対象外通信の隔離・3種類の終了時解除を実VMで確認済みです。** 通信は常に許可し、ペイロードやPIDは取得しません。週末の完成線は M3 の接続拒否・解除です。

## 始め方

Nix を利用します。mise は不要です。NixOS、Linux、Rust、Markdown リント、QEMU は `flake.lock` の固定した nixpkgs から取得します。

```sh
nix develop "path:$PWD"
./scripts/check.sh
```

Apple Silicon Mac では、構築用 VM を一つ目のターミナルで起動します。ホストの Nix 権限やシステム設定を変更しません。

```sh
./scripts/bootstrap-builder.sh
```

二つ目のターミナルで専用 VM をビルドして起動します。

```sh
./scripts/build-vm.sh
./scripts/run-vm.sh
```

三つ目のターミナルで、起動後の機能・範囲を確認します。

```sh
./scripts/ssh-vm.sh 'sudo warden-preflight'
```

初回ダウンロードには数 GiB 程度のディスク容量が必要です。VM の稼働メモリは構築用と専用 VM がそれぞれ最大 4 GiB です。実測した導入結果は [M0](milestones/00-sandbox/README.md) を参照してください。

## M1 パケットカウンタ

構築用・専用VMを起動した状態で、次を実行します。

```sh
./scripts/build-counter-vm.sh
./scripts/test-counter-vm.sh
```

[実行ガイド](docs/GUIDE.md#m1-のビルドと実行)にCLIの起動方法を記載しています。カウントはcgroup_skbが処理するSKB単位で、物理NICのフレーム数とは限りません。

## 文書

- [実行ガイド](docs/GUIDE.md)：前提、構築、検証、停止、復帰、トラブル対応。
- [開発ガイドライン](docs/GUIDELINES.md)：変更範囲、設計境界、検証、公開の条件。
- [安全性の根拠と限界](docs/SAFETY.md)：何が保護するのか、何を保証できないのか。
- [セキュリティ方針](SECURITY.md)：脅威モデル、機密情報、問題報告。
- [ライセンス](LICENSE.md)：MIT ライセンスと第三者の扱い。
- [貢献ガイド](CONTRIBUTING.md)：変更提案と必須確認。
- [ロードマップ](docs/ROADMAP.md)、[構成](docs/DIRECTORY_LAYOUT.md)、[アーキテクチャ](docs/ARCHITECTURE.md)。

## 開発時の検証

Markdown リント、Rust の整形・Clippy・アーキテクチャガード、Nix の設定評価を一括実行します。ガードは禁止した依存方向、カーネル/common の `std`/`alloc` 参照、M0 の対象・モード・資源制限を確認します。将来のランタイムコードは実装後に検査され、M0 でその正しさまで確認したとは扱いません。

VM の実動作は `warden-preflight` で別途確認します。静的チェックだけでは M0 の完成になりません。

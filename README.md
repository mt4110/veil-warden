# veil-warden

[English](README.en.md)

Rust と Aya で Linux VM 内の通信を観測し、事前ルールで新規接続を拒否する実験プロジェクトです。指定した合成テストプロセスだけを対象にします。

**M5まで完了し、専用NixOS VMで検証済みです。** M1は送信SKB数、M2はTCP IPv4/IPv6の接続試行とPID/TID・宛先を表示します。M3は明示した宛先ルールに一致する新規TCP接続を拒否・解除します。既定は監視のみです。既定では引数を取得せず、ペイロード・環境変数も取得しません。M5では明示した合成プロセス1件のargvだけを一度評価し、秘密値なしで警告します。

## 読みたい内容から探す

| 目的 | 入口 |
| --- | --- |
| まず動かす・停止する | [実行ガイド](docs/GUIDE.md) / [短いデモ](docs/DEMO.md) |
| 設計と安全性を理解する | [アーキテクチャ](docs/ARCHITECTURE.md) / [安全性](docs/SAFETY.md) |
| 制約や未解決事項を知る | [既知の課題](docs/KNOWN_ISSUES.md) / [M5の限界](docs/SECRET_WARNING.md) |
| 質問する・開発に参加する | [サポート](SUPPORT.md) / [貢献ガイド](CONTRIBUTING.md) |
| 進捗と今後の候補を確認する | [ロードマップ](docs/ROADMAP.md) / [研究テーマ](docs/RESEARCH.md) |
| すべての文書と実測記録を探す | **[ドキュメント一覧](docs/README.md)** |

Cloudflareへの登録・組織参加・デプロイは不要です。NixとローカルLinux VMで実行します。依存の取得にはインターネット接続を使います。

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

## M2 接続監視

両VMが起動した状態で実行します。M1と共通のビルドスクリプトを使います。

```sh
./scripts/build-counter-vm.sh
./scripts/test-connect-vm.sh
```

CLIの `connect` モードは接続試行を表示します。接続成功や通信の安全を意味しません。RingBufと表示キューの欠落数も表示します。[実行ガイド](docs/GUIDE.md#m2-接続監視の実行)と[M2の検証結果](milestones/02-connect-monitor/README.md)を参照してください。

## M3 接続拒否・解除

```sh
./scripts/build-counter-vm.sh
./scripts/test-policy-vm.sh
```

`connect --enforce --deny IP PORT` で事前ルールを指定します。稼働中は `policy add IP PORT`、`policy remove IP PORT`、`policy list` で操作します。すべて専用VM内のroot操作です。ルールは最大16件、IPv4-mapped IPv6も同じIPv4ルールに照合します。

IPv4/IPv6の拒否・解除、対象外通信、容量超過、既存接続の継続、3種類の終了後の復帰を実測しました。ログ欠落時も拒否判断は変わりません。既存接続・UDP/QUIC・秘密の流出防止は保証対象外です。[操作ガイド](docs/GUIDE.md#m3-接続拒否と解除の実行)と[M3記録](milestones/03-connect-policy/README.md)を参照してください。

## M3A デモとM4 TUI

[短いデモ](docs/DEMO.md)は監視・拒否・解除・終了後の復帰を5段階で確認します。

```sh
./scripts/demo-policy-vm.sh
./scripts/tui-vm.sh             # 監視のみ
./scripts/tui-vm.sh --enforce   # 明示した宛先の拒否・解除
```

TUIは接続履歴、拒否ルール、モード、欠落数を表示します。Tabと↑↓/j/kで選択し、bで拒否、dで解除を準備、宛先を確認してEnterで実行します。Escで取消、q/Ctrl+Cで終了します。履歴は128件に制限し、拒否状態を文字でも表示します。実端末から起動してください。[操作ガイド](docs/GUIDE.md#m4-tuiの実行)と[M4検証記録](milestones/04-tui/README.md)を参照してください。

## M5 シークレット警告

`connect --scan-argv PID:START_TICKS`で、専用cgroup内の指定プロセスだけを評価します。TUIでも評価状態・rule ID・件数を表示します。検知は送信の証拠ではなく、自動で拒否ルールを登録しません。未評価を検知なしと扱いません。最大16 KiB、1件、親の待機上限2秒です。

```sh
./scripts/test-secret-vm.sh
```

[仕様・安全性・制約](docs/SECRET_WARNING.md)と[M5検証記録](milestones/05-secret-warning/README.md)を参照してください。今回の研究はM5で区切り、M6の常駐化は保留します。

## 文書とOSSの案内

[ドキュメント一覧](docs/README.md)に全資料を目的別にまとめています。[変更履歴](CHANGELOG.md)、[セキュリティ報告](SECURITY.md)、[ライセンス](LICENSE.md)、[第三者の帰属](docs/third-party/veil-rs-MIT.txt)もこちらから辿れます。

## 開発時の検証

Markdown リント、Rust の整形・Clippy・アーキテクチャガード、Nix の設定評価を一括実行します。ガードは禁止した依存方向、カーネル/common の `std`/`alloc` 参照、M0 の対象・モード・資源制限を確認します。M1のフックが常に許可を返すこと、M3の両フックが共通の判断関数を使うこと、ユーザー側のproc参照がcomm等の承認した範囲であることも検査します。ガードだけで実動作を保証しません。

VM の実動作は `warden-preflight` で別途確認します。静的チェックだけでは M0 の完成になりません。

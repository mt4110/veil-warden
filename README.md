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

## クイックスタート

Apple Silicon Mac と Nix を用意し、このリポジトリで一度だけインストールします。

```sh
./scripts/install-cli.sh
```

新しいターミナルを開けば、どのディレクトリからでも次の1コマンドで監視画面を開けます。

```sh
veil-warden
```

RustでビルドしたMac用コマンドを `~/.local/bin/veil-warden` に配置し、`.zshrc`（`ZDOTDIR`設定時はその配下）へPATHを追記します。VMの起動・SSH待機・不足する成果物のビルド・preflightを自動で実行します。初回の依存取得は数GiBになる場合があります。ビルド結果は次回も再利用し、ソース更新後は `veil-warden stop`、`veil-warden build` で更新します。

VMはmacOSのlaunchdでバックグラウンド実行します。ターミナルを閉じてもVMは動作しますが、ログイン時の自動起動は行いません。監視本体は専用Linux VM内で動き、TUIの終了で監視を解除します。常時監視・ルール永続化を行うM6とは別の、起動手順の簡略化です。

| 操作 | コマンド |
| --- | --- |
| VMだけ準備・起動 | `veil-warden start` |
| 監視画面 | `veil-warden` |
| 明示した宛先の拒否・解除 | `veil-warden tui --enforce` |
| 合成通信のデモ | `veil-warden demo` |
| VMの状態 | `veil-warden status` |
| 管理するVMを停止 | `veil-warden stop` |

ヘルプは `veil-warden -h` で表示します。`veil-warden tui -h` のようにコマンド別の説明も確認できます。`--lang ja` または `--lang en` で言語を指定でき、省略時はロケールに合わせます（日本語以外は英語）。例: `veil-warden --lang en -h`。

既存の手動起動VMは再利用し、`stop`では停止しません。元のターミナルで停止してください。VMログは状態ディレクトリの `host-cli/builder.log` と `host-cli/sandbox.log` です。VMごとのメモリ上限は4 GiBです。リポジトリを移動した場合は再インストールします。既存のCLIはバックアップしてから更新し、ログ・ディスクは削除しません。

## 手動でVMを準備する場合（開発者向け）

Apple Silicon Mac と Nix を使います。mise は不要です。初回は数 GiB のダウンロードがあり、インターネット接続が必要です。

次の2ステップで専用VMを準備して起動できます。1つ目のターミナルは構築用VMとして開いたままにし、2つ目で専用VMのビルド・起動・preflightをまとめて実行します。ホストのNix権限やシステム設定は変更しません。

```sh
# 1. ターミナル1
nix develop "path:$PWD" -c ./scripts/bootstrap-builder.sh
```

```sh
# 2. ターミナル2
nix develop "path:$PWD" -c ./scripts/start-vm.sh
```

起動後のVMはバックグラウンドで動き、preflight結果を表示します。VMの出力は `$XDG_CACHE_HOME/veil-warden-m0/sandbox/quickstart-vm.log`（未設定なら `~/.cache/veil-warden-m0/sandbox/quickstart-vm.log`）に保存します。停止は `./scripts/ssh-vm.sh 'sudo poweroff'` です。詳しい復帰・トラブル対応は[実行ガイド](docs/GUIDE.md)を参照してください。

各VMのメモリ上限は4 GiBです。実測した導入結果は [M0](milestones/00-sandbox/README.md) を参照してください。

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

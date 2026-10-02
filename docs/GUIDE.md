# veil-warden M0 実行ガイド

## 前提

Apple Silicon Mac と既存の Nix を使います。初回の取得にはインターネット、数 GiB 程度の空き容量が必要です。Nix が未導入なら、公式の導入方法を確認してから導入してください。このプロジェクトのスクリプトはホスト権限を拡大しません。

全スクリプトを Nix shell 内から実行します。mise は使用しません。

```sh
nix develop "path:$PWD"
./scripts/check.sh
```

M0 の stable Rust と整形・リントは固定 nixpkgs 由来です。nightly/Aya/bpf-linker は M1 で追加するため、現段階の shell に含めません。

## 構築用 VM

```sh
./scripts/bootstrap-builder.sh
```

このターミナルを開いたままにします。公式のキャッシュ済み NixOS Linux builder を、ホスト設定を変更する installer を通さず起動します。launcher は固定した公式生成物の形を確認し、loopback・2 CPU・4 GiB・ディスク32 GiBへ変更します。形が変わった場合は起動前に拒否します。

SSH はホスト `127.0.0.1:32222`、既知の公開ホスト鍵を検証します。生成した秘密鍵は状態ディレクトリ内に保持し、VM には公開鍵だけを渡します。

## 専用 VM のビルドと起動

別ターミナルで同じ Nix shell に入り、実行します。

```sh
./scripts/build-vm.sh
./scripts/run-vm.sh
```

ビルド用に flake・Cargo 設定、config、infra、tests、tools をローカル構築用 VM へコピーします。Linux の中で kernel/initrd/Nix store のイメージを作り、ホストへ戻します。Nix daemon の trusted-user 設定や remote builder 設定は不要です。

専用 VM は `127.0.0.1:32223` で SSH を受け、root は一時的なメモリ領域です。最初の公開ホスト鍵を取得した後はその起動の鍵を検証します。2 CPU・4 GiB、ホストと共有するのは公開鍵ディレクトリだけです。

## 機能と通信範囲の検証

起動後、別ターミナルから実行します。

```sh
mkdir -p artifacts/m0
./scripts/ssh-vm.sh 'sudo warden-preflight' > artifacts/m0/preflight.json
```

次を確認します。

- ARM64、cgroup v2、kernel BTF。
- cgroup_skb、cgroup_sock_addr、RingBuf、HashMap、per-CPU Array の利用可能性。
- 管理用 SSH がテスト slice の外にあること。
- systemd がテストクライアントを専用 slice に置いてからソケットを作ること。
- IPv4/IPv6 loopback の合成文字列がサーバーに届くこと。

`bpftool feature probe` は一時的な BPF program/map による capability 試験を行います。自作の通信プログラムを attach する段階ではありません。cgroup BPF link の実 attach は M1 で検証します。

## 停止と復帰

```sh
./scripts/ssh-vm.sh 'sudo poweroff'
./scripts/run-vm.sh
```

専用 VM は再起動で宣言した初期状態へ戻ります。同じ preflight を再実行してください。ディスクやログを自動削除しません。

構築用 VM はそのコンソールで `shutdown now` を実行します。公開鍵・ローカル状態は保持されます。構築用ディスクの破損や手動再作成が必要なら、既存ディスクを勝手に削除せず、対象と復旧方針を確認してください。

## 状態とトラブル対応

状態の既定先は `$HOME/.cache/veil-warden-m0`、`WARDEN_STATE_DIR` で別の専用ディレクトリを指定できます。秘密鍵は private、公開鍵は public-keys、構築用状態は bootstrap、専用 VM の各起動は sandbox/runs に置きます。既存の user data を置かないでください。

ポートが使用中なら既存の VM を確認し、重複起動しません。SSH が起動前なら、コンソールで起動を確認してから再試行します。鍵検証に失敗したら検証を無効にせず、対象の起動と known_hosts を確認します。

Markdown/Rust/Nix の検証失敗は原因を修正してから再実行します。M0 の環境構築が3時間の作業枠で成立しない場合は結果と阻害要因を記録して停止します。

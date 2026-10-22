# Nix一体パッケージと非ロード診断：ARM64 Linux検証

[手順書](../../NIX_PREFLIGHT.md) / [文書一覧](../../README.md)

2026-10-22。専用Colima builder内でCLIとeBPFを一組にしたNixパッケージをビルドし、
BPFをロードせずに生成CLIの事前診断を検証しました。
これはパッケージと読み取り診断の確認であり、一般サーバーでの監視・保護の実証ではありません。

## 入力と環境

| 項目 | 記録 |
| --- | --- |
| 実行CPU | aarch64-linux。x86_64の実ビルド・実行は未実施 |
| Linux kernel | 6.8.0-117-generic |
| Nix | 2.33.1 |
| CLI toolchain | pinned nixpkgsのRust/Cargo 1.95.0 |
| BPF toolchain | nightly 2025-12-01、bpf-linker 0.9.15、bpfel-unknown-none / v3 |
| 対象 | 稼働アプリを置かない専用ビルド環境。cgroup作成・サービス登録なし |
| 入力 | PR作成前の固定作業snapshot。文書更新後のPR commitそのもののLinux再ビルドではない |

[主要入力のSHA-256](tested-inputs.json)は、ビルドしたsnapshotとこのPRのruntime/buildファイルが一致することを確認するための記録です。
文書は検証後に更新しています。CIでのビルドや、以後のcommitの結果は本記録から推定しません。
[成果物manifest](package-manifest.json)には、CLI/BPFの実SHA-256、CPU、イベントABI、source store pathを残しています。
store pathはローカル成果物の識別子で、一般に取得できる配布URL・署名・provenanceではありません。

## 結果

| 確認 | 結果 |
| --- | --- |
| ARM64 LinuxでのNix一体パッケージ | 実ビルド成功 |
| CLI/BPFのhash・ELF形式・CPU | パッケージ生成時と、ホストへ取り出した証拠コピーの両方で一致 |
| Linux CLIの単体テスト | 24成功、0失敗、既存2件skip |
| 実CLI診断の回帰テスト | 3件成功 |
| manifestの回帰テスト | 4件成功。改変、CPU不一致、未知ABI・不適切な対応主張を拒否 |
| 日英診断 | [日本語](diagnose-ja.txt) / [English](diagnose-en.txt)。終了1で現行loaderの対象外を表示 |
| ヘルプ・不正入力 | ヘルプは終了0、不正入力は終了2 |
| syscall追跡 | 日英診断・help・入力不正の4ケースで、BPF syscall・書込用open・network/mount/ファイル変更の試行なし |
| 後処理 | 専用コンテナとColimaを停止。他プロフィールの状態・容量設定は着手前と一致 |

skipした既存テストは、プロセス全体の資源制限を変更する隔離試験と、実PTYを必要とする端末復元試験です。
本確認のためにskipを追加したり、期待値や安全条件を弱めたりしていません。

[全体判定JSON](summary.json)、[trace確認](trace-check.json)、
[日本語trace](diagnose-ja.trace)、[英語trace](diagnose-en.trace)、
[help trace](diagnose-help.trace)、[不正入力trace](diagnose-invalid.trace)を添付しています。
traceは診断CLIの4実行だけが対象で、ビルド全体や全単体テストを追跡した記録ではありません。

## 診断の終了1は想定した結果

入力 `/system.slice/example.service` は現行の固定テストcgroupではないため、`loader_scope=blocked`です。
platform/kernel/cgroup v2の限定条件は`candidate`、実行環境で存在しない対象ディレクトリは`unknown`でした。
`runtime_acceptance=unknown`、`apply_supported=false`、`deployment_ready=false`を維持しています。

この結果は、サービスを指定すれば監視を開始できることを意味しません。
hostnameの変更や制限解除で実行する手順も提供しません。

## 非ロードで再確認する手順

Nix導入済みで利用が許可されたLinux上のGit checkoutを使います。
以下は開発用ビルドと診断だけで、VM起動、BPFロード、サービス登録を行いません。

```sh
nix --extra-experimental-features 'nix-command flakes' build .#veil-warden --option builders ''
nix --extra-experimental-features 'nix-command flakes' develop . --command python3 scripts/verify-package.py ./result
nix --extra-experimental-features 'nix-command flakes' develop . --command python3 tests/host/test_diagnose.py ./result/bin/veil-warden
./result/bin/veil-warden diagnose --cgroup /system.slice/example.service --lang ja
./result/bin/veil-warden diagnose --cgroup /system.slice/example.service --lang en
```

最後の2コマンドの終了1は不適合判定として期待します。自動処理で`|| true`を付けて合格扱いにしないでください。
未確認しかない場合も終了3で、適用許可にはなりません。

## 未検証・対象外

- x86_64の実ビルド・実行、他kernel/ディストリビューションでの対応認定。
- BPFロード、接続監視・拒否・解除、停止後の復帰。既存の専用VM試験とは別です。
- 正しいサービス所有者の同定、cgroup寿命・再作成、LSM・権限・他BPFプログラムとの共存。
- systemd登録、一般サーバー向けloader、SBOM・署名付き配布、更新・撤去。

旧証拠・旧bundleとソースは保全し、コード修正はせずにビルドと診断を確認しました。
生のビルドログ、停止確認、全入力snapshotはローカルのGit対象外`artifacts/`に保持し、ここには非機密の要約と検証対象を記録しています。
Keeper Cの試験は、この検証や判定に含めていません。

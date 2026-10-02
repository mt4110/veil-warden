# M1 パケットカウンタ

状態：完了（2026-10-02、実VMで検証済み）。前提：M0。

## 到達点

専用cgroupの送信SKB数をRust CLIで表示する。通信は常に許可し、payload・IP・PIDは収集しない。

## 実装

- [x] Aya templateを参考に、依存/toolchainを固定する。
- [x] cgroup_skb egressと1エントリのper-CPU u64 counterを使い、必ず許可を返す。
- [x] 一定間隔でCPU別値の合計と前回との差分をCLI表示する。
- [x] FD所有のリンクをpinせず保持し、終了時に解放する。
- [x] 固定VM・固定cgroup・Linux >= 5.7を確認し、二重起動を拒否する。

実装先は `crates/veil-warden-common`、`crates/veil-warden-ebpf`、`crates/veil-warden`、`tests/vm/cases/packet_counter.py` です。macOSではCLIの引数試験とガードを行い、ロードはLinuxだけで実行します。

## 合格条件と結果

- [x] IPv4/IPv6の合成loopback通信が受信され、対象counterが増える。
- [x] 対象外クライアントだけの同じ通信ではcounterが増えない。
- [x] 通常終了・SIGTERM・SIGKILL後に自作linkとcgroup attachmentが残らない。
- [x] 既存systemd BPFのアタッチ種別・フラグ・名前が保持され、管理SSHが維持される。
- [x] root cgroup指定、存在しないobject、二重起動を拒否し、実行中linkを変えない。

| 終了方法 | IPv4の増加 | IPv6の増加 | 対象外通信の前→後 | 自作link解除 |
| --- | --- | --- | --- | --- |
| 通常終了（250 samples） | 8 | 8 | 16→16 | 確認済み |
| SIGTERM | 8 | 8 | 16→16 | 確認済み |
| SIGKILL | 8 | 8 | 16→16 | 確認済み |

各familyにつき8個の固定合成UDPデータグラムを使用しました。今回の入力では8増えましたが、一般にアプリの送信回数・物理フレーム数との完全一致は保証しません。全CPU値の同時snapshotや高負荷時の厳密な計数・性能は未検証です。

systemdはtransient unitの生成・終了に伴って自身のfirewall programを再生成するため、program IDは変わります。試験ではkernelが割り当てたID以外の全attachment fieldを比較し、自作linkのID消失と併せて確認します。既存programの解除・置換をCLIから行う処理はありません。

## 固定版

| 対象 | 採用版 |
| --- | --- |
| NixOS / kernel | 26.05 / 6.18.54、aarch64 |
| ユーザー空間Rust | stable 1.95.0 |
| BPF Rust | nightly-2025-12-01、1.93.0-nightly、LLVM 21.1.5 |
| bpf-linker | 0.9.15、固定nixpkgs由来 |
| Aya / aya-ebpf | 0.14.0 / 0.2.1 |
| Template参考revision | c0fd79891b5ac8f73c1092bc2a374268d0548c7c |
| rust-overlay revision | 368fee9beaab04ca6fe7af28db63caa9badb22fa |

NixOS/kernelはM0の固定を保持しています。nightlyとlinkerはLLVM majorを合わせ、Cargo.lockは推移依存も固定します。テンプレートgeneratorは実行せず、M1に必要な構成だけを作成しました。一次資料は [参照資料](../../docs/SOURCES.md) を参照してください。

Aya 0.14のattach mode引数はSingle（flags=0）を使用します。LinuxのBPF_LINK_CREATEは内部でALLOW_MULTIを追加するため、既存のsystemd programと共存します。ALLOW_MULTIをlink_createの入力flagsに渡すとEINVALになることを実VMと固定kernelのソースで確認しました。legacy PROG_ATTACHへ進まないよう、attach前にkernel versionを確認します。

## 検証と証拠

基準commitはM0の `2729cf0`、M1変更は未コミットの作業ツリーです。

- macOS ARM64とLinux ARM64でRust guard 10件、CLI 2件が成功。
- Python公開鍵境界2件、Markdown lint、Rust fmt/Clippy、ShellCheck、Nixfmtが成功。
- macOS上の `nix flake check` が成功。Linux CLI/BPFはローカルbuilderで実ビルド。
- `build-counter-vm.sh` と `test-counter-vm.sh` を実行し、最終ソースから再現。
- cargo-audit 0.22.1：RustSec DB revision `117edb3bed98e9be112f277b7615eea3252e7c43` に対し既知脆弱性0件、警告0件。
- Cargo metadataの全61packageにlicense宣言あり。主にMIT/Apache-2.0、ほかZlibおよびUnicode-3.0。配布時の第三者notice確認は別途必要。

ローカル証拠は `artifacts/m1/acceptance-20261002T191431-79997.json`、`linux-checks.log`、`build-script.log`、`audit.json` に保持しています。再実行時は日時・PIDを付けたJSONを作ります。GitHub CI設定にBPF/CLIビルドを追加しましたが、remote CIは未実行です。

## 中止と対象外

ロード/attach失敗では採用版とverifier診断を確認し、安全条件を緩めない。PID取得、宛先表示、TCP接続監視、TUI、遮断、secret scanは追加していません。root管理者や任意の持込BPF objectに対する安全性保証ではありません。

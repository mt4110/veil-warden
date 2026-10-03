# veil-warden 開発ガイドライン

## 目的と変更範囲

当初の完成線M3に加え、TUIと明示argv警告まで実装しました。今回の研究はM5で区切ります。[研究候補](RESEARCH.md)の掲載だけで新規実装を開始しません。主力プロダクトを止めるための新規事業とは扱いません。M0 の範囲は環境・復帰・検証基盤だけです。

新しい技術や機能は、何を検証するか、終了条件、現在の成果への接続を明確にしてから扱います。TUI、シークレット検知、常駐化、全 OS の監視を M0 のついでに追加しません。

## アーキテクチャ境界

- common は固定幅型と共有仕様を持ち、no_std を維持する。
- eBPF は common に依存できるが、ユーザー空間 crate に依存しない。std/alloc を使わない。
- ユーザー空間は common に依存し、eBPF を通常のライブラリとして依存にしない。ビルド成果物として扱う。
- loader は Linux の資源を所有し、リンク・Map の寿命と終了を管理する。
- アーキテクチャガードは検証用の独立ツールで、監視ランタイムには依存させない。

現在、実装済み workspace は `tools/architecture-guard` だけです。将来の3クレートを先に空で作って完成を装いません。`config/architecture.toml` が workspace 内で許可する依存方向を定義します。ガードは Cargo の renamed/target-specific 依存も確認し、Rust の構文を解析して std/alloc の参照を検出します。

これは完全な静的解析器ではありません。macro が生成するコード、外部依存の挙動、OS の権限、実行時の寿命は別途ビルド・レビュー・実験で確認します。

## バージョン管理

M0 は `flake.lock` の nixpkgs revision と narHash、`Cargo.lock` の解決結果を固定します。mise と floating latest は使用しません。`rust-toolchain.toml` は Nix 外の利用でも同じ stable 版を選ぶための補助です。

NixOS の `system.stateVersion` は互換性の基準であり、kernel や package の版固定ではありません。kernel と package は固定 nixpkgs と明示した package set から選びます。

M1 の nightly Rust、aya/aya-ebpf、bpf-linker はまだ未選定です。採用時に公式 API、対応 LLVM、ライセンス、実カーネルでの動作を確認し、固定してから使います。

## 安全条件

`config/sandbox.toml` は M0 の対象・モード・資源の共有仕様です。Nix の assertions と Rust ガードで検査し、launcher でも範囲を確認します。正常な検証を通すために境界を緩めません。

秘密鍵はローカル状態ディレクトリに留め、Nix store や VM へコピーしません。環境構築のネットワーク取得と、ユーザーデータの外部送信を区別します。実験は loopback の合成データを使います。

## 検証と完成

静的検証は整形、Clippy、Rust ガード、Markdown リント、Nix 評価です。VM 検証は ARM64、cgroup v2、BTF、必要な BPF program/map、管理用接続の範囲、IPv4/IPv6 の受信、復帰です。

実験用クライアントは systemd が専用 slice に配置してからソケットを作ります。フックの有無だけでは通信制御の正しさを証明しないため、M3 ではクライアントの結果とサーバー受信を両方確認します。

ガードのテスト数、VM の起動、ビルド成功はそれぞれ異なる証拠です。M0 が通っても M1〜M3 の通信監視・拒否は未検証です。完成報告には実施結果と未検証を分けて記載します。

## M3 ルール操作の検証

既定observeと明示enforceを区別し、ルール一致だけで新規TCP接続を拒否します。keyのfamily/IP/port/protocol、IPv4-mapped IPv6正規化、ABIのactionとpolicy ID整合を純粋Rustテストで確認します。M1の無条件許可ガードを維持し、M3の両フックは共通の判断処理を使うことを構造検査します。

実VM試験では拒否クライアントのerrno、受信側のデータゼロ、解除後の両familyの接続、対象外・異なるIP/port・UDPを確認します。Map上限と重複/不存在の操作は失敗し、実Mapの現状が変わらないことを確認します。既存接続の継続と、ログ欠落中の拒否継続も別々に検証します。モックや静的検査でこれらを代用しません。

## M4の検証

描画内容・有界履歴・確認中tupleの保持はTestBackendと純粋状態テストで確認します。Linux PTYでは実キー、リサイズ、Map更新、errno、termiosとalternate screenの復元、BPF link解除を確認します。描画失敗はテスト専用writerで発生させ、ignored fixtureをPTY内で明示実行します。通常のテストでignoredだったことを実行証拠にしません。

## M5の扱い

M5は`--scan-argv PID:START_TICKS`で指定した専用cgroupの合成プロセス1件を、attach前に一度だけ評価します。既定ではargvを読みません。値を含むFindingを生成せず、rule ID・件数・評価状態だけを返します。検知からルールを追加せず、未評価と検知なしを区別します。資源上限、PID/FDの確認、既知の見逃しと復帰手順は[仕様と限界](SECRET_WARNING.md)を参照してください。

# M4 TUI

状態：完了（2026-10-02、専用NixOS VMの実PTYで検証）。前提：M3/M3A。

## 到達点

接続履歴、拒否ルール、欠落数、現在モードを端末で確認する。

## 実装先

user/tui.rs、cli.rs、events.rs、policy.rs、output.rs。詳細は [ディレクトリ構成](../../docs/DIRECTORY_LAYOUT.md) を参照。

## 作業

- [x] Ratatui と端末 backend の採用版を確認する。
- [x] メイン画面を接続履歴とルール表示に分け、監視範囲を常に示す。
- [x] 選択した宛先への拒否/解除は対象 tuple を表示して操作する。
- [x] 受信・描画・名前解決を分け、有界履歴と終了時の端末復帰を実装する。

## 合格条件

- [x] キーボード操作、q/Ctrl+C、リサイズ、日本語表示を確認する。
- [x] 空状態、読み込み、受信エラー、更新失敗、欠落を識別できる。
- [x] 描画/起動エラー、正常終了後に raw mode と alternate screen が復帰する。
- [x] 入力があっただけで終了せず、拒否状態を色だけに依存せず示す。

## 中止と対象外

2 時間の追加枠で未完成なら M3 の CLI を週末成果とする。daemon/TUI 間 IPC は作らない。

## 検証記録

基準commitはe0debf0。M3A/M4はその後の未コミット差分。Apple Silicon Mac上の独立NixOS VM（aarch64、Linux 6.18.54、2 CPU / 4 GiB）を使い、Rust 1.95.0、Ratatui 0.30.2、Crossterm 0.29.0を固定した。BPFのnightly/AyaはM3から変更していない。

`./scripts/test-tui-vm.sh` で実PTYのキー入力・Map・クライアント・termios・ANSI画面状態・BPFリンクを確認した。TestBackendの描画試験はPTY試験と区別する。

| 確認 | 結果 |
| --- | --- |
| observe | 空状態と準備表示、実接続の履歴、拒否操作の無効表示。Mapは0件 |
| enforce | bだけではMap不変、Escで取消、Enter後に実Map1件。新規接続はEPERM、DENYを文字で表示 |
| 解除 | Tab/d/Enterで実Map0件、接続成功 |
| 更新失敗 | 重複・実Map16件の容量超過で更新失敗表示。Mapの現状を維持 |
| 入力・リサイズ | 普通のキーだけでは終了しない。120×32、70×24、40×10、元サイズへの復帰を確認。日本語の実画面snapshotを保存 |
| 通常終了 | q、Ctrl+C、SIGTERM、期限付き終了でtermiosとalternate screen復元、link解除 |
| 起動失敗 | object欠落、片方のattach失敗で非ゼロ終了、端末復元、link解除 |
| 描画失敗 | テスト専用writerがdrawで実エラー。ignored fixtureをPTY内で1件実行し、SessionのDropによる端末復元を確認 |
| 欠落 | 4,096合成接続、ring欠落3,841を画面表示。終了後の復帰も確認 |
| SIGKILL | link解除と接続復帰。端末は自動復元されないことを実測し、親PTY側のtermios/表示復元も確認 |
| 非TTY | attach前に明確なエラーで拒否 |

純粋Rust試験で履歴128件の上限、確認中tupleの保持、observe禁止、更新失敗状態、日本語・空状態・受信エラー表示を確認した。Ratatui/Crosstermはユーザーcrateだけに置き、TUIがdaemonの制御clientを呼ばないことをRustの構造ガードで確認する。

通常のLinuxテストではPTY専用fixtureをignoredとし、PTY試験で明示実行したことを検証する。テスト数を品質保証として扱わない。ANSI解析器は本試験のsubsetであり、実端末の全挙動を再現するemulatorではない。

未検証：Windows/macOSでのBPFロード、実端末のすべてのfont・描画環境、長時間運用・性能、terminal消滅/abort時の復元。SIGKILL後の端末復元を保証しない。CLIと同様に既存接続、UDP/QUIC、TLS復号、秘密の流出防止は対象外。常駐サービスとdaemon/TUI IPCは追加していない。

操作は [実行ガイド](../../docs/GUIDE.md#m4-tuiの実行)、影響範囲は [安全性文書](../../docs/SAFETY.md#m4-操作と端末復元) を参照。

ローカル証拠： `artifacts/m4/acceptance-20261002T222027-30236.json` と `artifacts/m4/enforce-screen.txt`（Git対象外）。

最終版でM1/M2/M3のCLI回帰、M3Aデモ、macOS/LinuxのClippy・Rustテスト、Markdownlint・shellcheck・Nix flake checkが通過した。コミット・push・GitHub CI実行はこの作業では行っていない。

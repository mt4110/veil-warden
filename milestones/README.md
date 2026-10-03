# veil-warden マイルストーン

順に M0〜M3 を完成させる。M4とM5も完了した。今回の研究はM5で区切り、M6は保留。各段階のチェックは、対応する検証を実際に行ったときだけ更新する。

| 段階 | 文書 | 状態 |
| --- | --- | --- |
| M0 | [サンドボックス](00-sandbox/README.md) | 完了、VM実測済み |
| M1 | [パケットカウンタ](01-packet-counter/README.md) | 完了、VM実測済み |
| M2 | [接続監視](02-connect-monitor/README.md) | 完了、VM実測済み |
| M3 | [接続拒否と解除](03-connect-policy/README.md) | 完了、VM実測済み |
| M3A | [完成整理](03a-finish/README.md) | 完了、5段階デモ実測済み |
| M4 | [TUI](04-tui/README.md) | 完了、VM/PTY実測済み |
| M5 | [シークレット警告](05-secret-warning/README.md) | 完了、VM/PTY実測済み |
| M6 | [サービス化](06-service/README.md) | 保留、今回の研究対象外 |

検証結果には commit、VM/kernel/toolchain/依存版、入力条件、実行結果、対象外と未検証事項を記録する。実験用 IP と合成プロセスのみ使い、秘密や実 argv を記録しない。ローカル artifacts は実装時に Git 対象外とする。必要な非機密の結果概要だけ各 README に追記する。

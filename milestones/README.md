# veil-warden マイルストーン

順に M0〜M3 を完成させる。M4 は追加目標、M5 と M6 は後日の独立作業。各段階のチェックは、対応する検証を実際に行ったときだけ更新する。

| 段階 | 文書 | 状態 |
| --- | --- | --- |
| M0 | [サンドボックス](00-sandbox/README.md) | 完了、VM実測済み |
| M1 | [パケットカウンタ](01-packet-counter/README.md) | 完了、VM実測済み |
| M2 | [接続監視](02-connect-monitor/README.md) | 未着手 |
| M3 | [接続拒否と解除](03-connect-policy/README.md) | 未着手 |
| M4 | [TUI](04-tui/README.md) | 未着手 |
| M5 | [シークレット警告](05-secret-warning/README.md) | 未着手 |
| M6 | [サービス化](06-service/README.md) | 未着手 |

検証結果には commit、VM/kernel/toolchain/依存版、入力条件、実行結果、対象外と未検証事項を記録する。実験用 IP と合成プロセスのみ使い、秘密や実 argv を記録しない。ローカル artifacts は実装時に Git 対象外とする。必要な非機密の結果概要だけ各 README に追記する。

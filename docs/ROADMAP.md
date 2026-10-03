# veil-warden ロードマップ

[README](../README.md) / [文書一覧](README.md) / [課題一覧](KNOWN_ISSUES.md) / [研究テーマ](RESEARCH.md)

## 目的と現在の終了点

LinuxのネットワークフックをRustで扱い、限定されたVM内の観測・事前ルールによる拒否・復帰を確認する研究用OSSです。主力プロダクトを止める事業開発とは扱いません。収益・需要・本番DLPへの適合性は未検証です。

当初の完成線はM3でした。その後、M3Aの完成整理、M4のTUI、M5の明示argv警告を個別に承認して実装・検証しました。今回の研究はM5で区切り、M6は保留します。新しい研究・常駐化・公開・販売へ自動的に進みません。

## 完了した範囲と保留

| 段階 | 到達点 | 現在の状態 |
| --- | --- | --- |
| [M0](../milestones/00-sandbox/README.md) | ARM64専用NixOS VMと復帰・検証基盤 | 完了、VM実測済み |
| [M1](../milestones/01-packet-counter/README.md) | 常に通信を許可する送信SKBカウンタ | 完了、VM実測済み |
| [M2](../milestones/02-connect-monitor/README.md) | IPv4/IPv6のTCP接続試行をCLIへ表示 | 完了、VM実測済み |
| [M3](../milestones/03-connect-policy/README.md) | 事前tupleルールで新規接続を拒否・解除 | 完了、VM実測済み |
| [M3A](../milestones/03a-finish/README.md) | 差分と証拠の整理、5段階デモ | 完了、VM実測済み |
| [M4](../milestones/04-tui/README.md) | 接続履歴・ルールのTUI表示と確認操作 | 完了、VM/PTY実測済み |
| [M5](../milestones/05-secret-warning/README.md) | veil-rsの6ルールで明示argvを一度評価、値なし警告 | 完了、VM/PTY実測済み |
| [M6](../milestones/06-service/README.md) | 限定scopeのobserveサービス | 保留、未着手 |

完了は各段階の合成試験の範囲です。公開リリース・一般環境対応・商用保護の完成とは区別します。初期の作業枠はM0〜M3で11時間、M4を含め13時間の上限案でした。実測所要時間や性能の保証として使いません。

## 成果を確認する方法

[短いデモ](DEMO.md)で監視 → 拒否 → 解除 → 再拒否 → 終了後の復帰を確認できます。M5は[シークレット警告の仕様](SECRET_WARNING.md)と専用の合成試験を使います。イベント表示だけで接続成功・受信の有無・漏洩防止を判定しません。

## 次の判断

現在の[課題](KNOWN_ISSUES.md)を把握し、文書とデモを使える成果として維持します。研究を追加するなら、[研究テーマ](RESEARCH.md)の問い・最小検証・終了条件・成果への接続を確認し、着手範囲と資源予算を別に決めます。今の成果を整理して公開する案や、追加実装を行わない案も比較します。

対象外通信への影響、秘密の露出、終了後の遮断残留があれば実験を停止します。root cgroupへの拡大、検知からの自動拒否、全秘密の保護は現在の計画に含めません。

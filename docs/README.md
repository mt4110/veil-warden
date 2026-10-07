# ドキュメント一覧

[日本語README](../README.md) / [English README](../README.en.md)

初めて動かす場合は「利用と検証」、変更を提案する場合は「開発と貢献」から読んでください。現在はM0〜M5が実装済みの研究用ツールです。実行手順、検証記録、今後の候補を別々に扱います。

## 利用と検証

| 文書 | 内容 |
| --- | --- |
| [利用場面と模擬TUI](USE_CASES.md) | 何に使うか、Macのdry-run、操作例、Wireshark等との使い分け |
| [実行ガイド](GUIDE.md) | Nix環境、VMの構築・起動、CLI/TUI、停止・復帰、トラブル対応 |
| [構築VMの保守](BUILDER_MAINTENANCE.md) | QEMU/HVF、ディスク容量、共通ビルドキャッシュ、整理と復旧の方針 |
| [短いデモ](DEMO.md) | 監視 → 拒否 → 解除 → 再拒否 → 終了後の復帰 |
| [シークレット警告](SECRET_WARNING.md) | M5の操作、対象指定、評価状態、資源上限、検知の制約 |
| [VM試験](../tests/vm/README.md) | 各受入試験の前提と確認範囲 |
| [既知の課題](KNOWN_ISSUES.md) | 現在の制約、未検証事項、対応候補 |
| [サポート](../SUPPORT.md) | 質問・不具合の報告先、添付する情報、対応範囲 |

Cloudflareのアカウント登録・組織参加・デプロイは不要です。現在の実行経路はNixとローカルLinux VMです。依存のダウンロードにはネットワーク接続を使います。

## 設計と安全性

| 文書 | 内容 |
| --- | --- |
| [導入と更新の設計候補](DEPLOYMENT.md) | 未実装のsystemd/DaemonSet案、リンクとMapの保持、更新・復帰の条件 |
| [アーキテクチャ](ARCHITECTURE.md) | フック、ABI、Mapとリンクの所有、制御・表示・スキャンの境界 |
| [ディレクトリ構成](DIRECTORY_LAYOUT.md) | ファイル配置とクレートの責務 |
| [安全性の根拠と限界](SAFETY.md) | VM・Verifier・cgroupが減らすリスクと保証できないこと |
| [セキュリティ方針](../SECURITY.md) | 脅威モデル、機密情報、非公開の問題報告 |
| [一次資料と設計評価](SOURCES.md) | 技術上の根拠、初期案から修正した主張 |

## 開発と貢献

| 文書 | 内容 |
| --- | --- |
| [貢献ガイド](../CONTRIBUTING.md) | 変更提案、確認、PRの説明 |
| [開発ガイドライン](GUIDELINES.md) | 変更範囲、設計境界、検証と公開の条件 |
| [変更履歴](../CHANGELOG.md) | 未リリースの機能概要と、今後の公開履歴の記録方法 |
| [ライセンス](../LICENSE.md) | MIT本文と第三者の扱い |
| [veil-rsのMIT帰属](third-party/veil-rs-MIT.txt) | 固定したエンジンのライセンス本文 |

## 進捗と研究

| 文書 | 内容 |
| --- | --- |
| [ロードマップ](ROADMAP.md) | 完了した範囲、現在の終了点、保留している作業 |
| [研究テーマ](RESEARCH.md) | 検証する問い、最小の試験、終了条件、成果への接続 |
| [マイルストーン一覧](../milestones/README.md) | M0〜M6の状態と各段階への入口 |

研究テーマの掲載は着手の承認や実装予定ではありません。現在の研究はM5で区切り、M6は保留しています。

## 段階別の実測記録

- [M0 サンドボックス](../milestones/00-sandbox/README.md)
- [M1 パケットカウンタ](../milestones/01-packet-counter/README.md)
- [M2 接続監視](../milestones/02-connect-monitor/README.md)
- [M3 接続拒否と解除](../milestones/03-connect-policy/README.md)
- [M3A 完成整理とデモ](../milestones/03a-finish/README.md)
- [M4 TUI](../milestones/04-tui/README.md)
- [M5 シークレット警告](../milestones/05-secret-warning/README.md)
- [M6 サービス化の保留仕様](../milestones/06-service/README.md)

生のローカル検証結果はGit対象外の`artifacts/`に置き、非機密の要約を各マイルストーンへ記録します。静的検査とVM実測、検証済みと未検証を混同しないでください。

# 企業向け候補のタスクと完了条件

[設計](ENTERPRISE_READINESS.md) / [マイルストーン](../milestones/enterprise/README.md)

2026-10-22。以下はローカルの計画IDで、GitHub Issue番号ではない。
今回の承認は文書更新だけ。全実装タスクは未着手、W0以降の実行・公開・試用は別途判断する。
Keeper Cのタスク・証拠は混ぜない。M0〜M5の完了をこれらへ繰り上げない。

| ID | 段階 | 依存 | 作業と完了条件 |
| --- | --- | --- | --- |
| W0-1 | 採否 | なし | [ ] 利用者・対象サービス・対応候補OSを一つずつ決め、既存手段では足りない点を確認。不要なら追加実装を終了 |
| W0-2 | 対象固定 | W0-1 | [ ] commit/CLI/BPF/ABI/CPU/kernel/cgroupの対応表と、管理経路・停止契約・試用終了条件を合意 |
| W0-3 | 説明と証拠 | W0-2 | [ ] [ビジョン](ENTERPRISE_VISION.md)に沿って公開説明を照合。100%安全・全Pod・非特権化済み・自動秘密遮断・未確認Release・監査認証を主張しない |
| W1-1 | M6前半 | W0-2、実装承認 | [ ] JSON Lines schemaとCLI契約。起動/イベント/統計/終了/異常の全行parse、unknown保持、値非露出、欠測を0へ変換しない回帰を通す |
| W1-2 | M6前半 | W1-1 | [ ] 出力詰まり・broken pipe・欠落・数値境界・worker終了を検証。接続判断と表示を分離し、有限期限と終了状態を証明 |
| W1-3 | M6後半 | W1-2 | [ ] 一つのscopeを対象に、既定disabled/observeのsystemd serviceを作る。必要権限・両link成立後ready・scope不適合拒否を確認 |
| W1-4 | M6後半 | W1-3、VM試験承認 | [ ] 新規VMで開始/停止/再起動/二重起動/読取失敗/SIGTERM/SIGKILLを確認。管理通信維持、秘密非出力、再起動loopなし、停止で解除を記録 |
| W2-1 | 配布 | W1-4 | [ ] 一つの対応OS/CPUでmusl/glibc候補を比較。実ELFの動的依存と有効featuresを調べ、CLI+BPF/ABI/版の一組を固定 |
| W2-2 | 配布前信頼 | W2-1 | [ ] 同一digestへSBOM・帰属・元commit/workflow/toolchain/provenanceを対応付け、スキャン結果と未解消事項をレビュー |
| W2-3 | 配布前信頼 | W2-2 | [ ] 署名identity/issuerとdigestの検証を用意。改変、別workflow、欠落署名を拒否。公開は別途承認 |
| W2-4 | 限定配布 | W2-3 | [ ] 対象上でcargo/Nixなしに取得・検証・診断・observe・停止。初回時間、失敗時非適用、削除/更新/旧版復帰を記録 |
| W2-5 | 条件付きdeb | W2-4、利用者要求 | [ ] packageの登録・upgrade/removeを新規VMで確認。既定disabled、インストールだけで起動/attach/enforceしない。rpmは要求がなければ実装しない |
| W3-1 | policy | W2-4、必要性 | [ ] 一形式とliteral TCP tupleのschema、scope/mode、重複・容量・権限・不正入力時の旧版維持を定義/検証 |
| W3-2 | policy Audit | W3-1 | [ ] 同じ候補ルールでwould-denyと実denyを区別し、Auditは接続を通す。不成立/イベント欠落を含め再現。既存observeと混同しない |
| W3-3 | policy更新 | W3-2 | [ ] 明示reload、単一更新者、世代と失敗結果を実装。部分書込・容量不足・片family失敗・同時接続中の混在と復旧を確認。notifyは保留 |
| W4-1 | metrics採否 | W1-4、試用者要求 | [ ] JSON/既存collectorで不足する運用項目と、HTTP同居/外部exporterの費用・権限・保守を比較。不要なら実装しない |
| W4-2 | metrics | W4-1採用 | [ ] 接続試行/欠落/readinessを有限labelで公開。HTTPは明示loopback、低速scrape・port競合・reset・欠測・変換境界を検証 |
| W4-3 | 監視連携 | W4-2 | [ ] 実collectorで受信・欠落alert・停止検出を確認。検証した接続先だけを対応表に記載 |
| W5-1 | K8s保留 | 単体試用と要求 | [ ] Pod/cgroup寿命、権限、対象node、CNI/他agent共存、更新/退避/復旧を設計してからDaemonSet/Helmの採否を決める |
| W5-2 | 更新保留 | 別の故障契約への合意 | [ ] DEPLOYMENTのlink/Map/ABI/世代/片family失敗/独立解除経路を検証。無停止・ログ欠落ゼロを先に宣言しない |

W4はW3に機械的に依存せず、要求があればW1後に採否を決められる。ただし同時P0にはしない。
最初に実装を選ぶならW0の確認後のW1-1。一括で全機能の実装・公開を約束しない。

各タスクの受け入れ記録には、対象commitと成果物hash、環境、正常/否定対照、
native終了、受信側の結果、対象外通信、停止と復旧、残る制約を含める。
文書チェック、単体fixture、fresh build、実VM試験、第三者試用は別々に記録する。

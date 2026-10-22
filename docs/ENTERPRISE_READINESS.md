# 企業での限定試用に向けた設計候補

[ビジョンと主張の境界](ENTERPRISE_VISION.md) / [ロードマップ](ROADMAP.md) / [タスク](ENTERPRISE_TASKS.md) / [段階と終了条件](../milestones/enterprise/README.md) / [導入・更新](DEPLOYMENT.md)

状態：文書設計のみ承認。機能実装・配布・本番導入は未承認、未実施。
2026-10-22、調査基点 `7589858ee0df2ae9609f62bafc32fe8e1a251891`。

最終ゴールは、対象・制約・停止時の挙動を理解した管理者が、一つのLinuxサービスへ安全に導入・運用・復旧できること。
現在はM0〜M5の限定VM実証まで。企業への一般提供、第三者試用、収益性は未検証。
具体的な対象企業・業務サービス・対応ディストリビューションは未決定。
KeeperのC試験は別プロジェクト・別証拠であり、Wardenの完了条件へ移さない。
本計画を主力プロダクトに優先するP0や、M6の実装開始とは扱わない。

## 1. 実装・証拠と提案を分ける

| 項目 | 現在の根拠 | 不足・誤解しやすい点 |
| --- | --- | --- |
| Linux / eBPF | `connect.rs` のconnect4/connect6、固定cgroup、新規TCP tuple拒否 | 全パケット/全プロセス/UDP/既存接続を制御しない。macOSネイティブ防御ではない |
| ビルド | `flake.nix`、`scripts/build-counter.sh`、lock、CI定義 | 本番でNixを使わず動く配布物の互換性・署名・Release公開は別の確認 |
| 設定 | `config/sandbox.toml` はNix/VM構成。CLIと実行時socketで単一tuple追加・解除 | 実行時ポリシーファイル、reload、CIDR、世代別一括反映ではない |
| observe | 起動時の既定mode、observe中のルール更新は拒否 | 「同じ拒否policyに一致したらwould-denyを出して通すAudit」は未実装。動的mode切替も未実装 |
| 統計 | `output.rs::Snapshot`、`events.rs` のMap合算・非同期処理 | attempted/deniedは接続試行の数。スキャンしたプロセス数や遮断パケット数ではない |
| 表示 | text出力とTUI、Macの模擬TUI | JSON Lines契約、metrics、非対話サービスは未実装。TUIは調査・実験の入口として維持 |
| 停止 | FD所有リンクを解放。停止後は拒否が失われる契約 | fail-open。bpffs保持・再接続・無停止更新は未実装 |
| OSS文書 | MIT、第三者帰属、SECURITY.md、既知の制約 | セキュリティ認証や監査合格ではない。SBOM・配布署名の代わりにもならない |

`.github/workflows/checks.yml` は静的検査/Nixチェック/BPF・CLIビルドの定義。
今回リモートCIの実行結果、最新Releaseや署名を照会していない。
同名バージョンや過去のCI成功を、現在の配布物の証拠にしない。

### 既存のVM検証

M0〜M5の範囲は各 `milestones/` の記録を参照する。
隣接Arenaの `docs/validation/warden-policy-20261022-v3.json` も確認した。
Mac ARM64/QEMU TCG、固定bundleでの許可・拒否・解除・停止復帰と対象外通信の記録で、
所有するcached CLI/BPFを再利用した試験。現在ソースのfresh build、x86_64、一般Linux、
PHP/CVE、クラウド、負荷、企業運用の受け入れ証拠にはしない。
Arenaの他方式・Keeperの成功をWardenへ転用しない。

## 2. 提案の採否

| 提案 | 判断・時期 | 最小の採用形 |
| --- | --- | --- |
| 非対話JSONログ | 有力、W1（既存M6の前半） | まずイベント・統計・準備状態・終了を型付きJSON Linesで出す |
| systemd常駐 | 有力、W1後半 | 一つの検証済みcgroup、既定observe、停止で解除。自動enforceなし |
| ビルド済みLinux配布 | 有力、W2 | 検証済みの1 OS/CPUからCLI+BPF object+文書を配布。対象上でbuild不要 |
| SBOM・署名・provenance | W2の配布前条件 | v1.0を待たない。同じ成果物digestに結び付け、検証失敗を検出 |
| `.deb` | W2の条件付き候補 | 対応OSを決めてから。登録しても既定disabled、無断起動/attachなし |
| `.rpm` | 保留 | 利用者の対象OSが必要とする場合だけ。最初から両形式を維持しない |
| 宣言的ポリシー | 有力、W3 | まず有限のTCP宛先tupleと一つのscope。形式は一つに絞る |
| policyのAudit | W3で明示的に設計 | would-denyと実拒否を区別。今のobserveをAuditと改称しない |
| Prometheus | 条件付きW4 | JSONだけで試用者の運用が足りるか確認して採否。HTTP依存を先に追加しない |
| DaemonSet/Helm/Scratch | W5として保留 | 単体サービスの試用とPod/cgroup寿命の検証後。極小imageは採用条件ではない |
| bpffs・ホットスワップ | W5として保留 | 現在と違う故障契約。リンク/Map/ABI/解除経路を検証してから |

「5分で検証」は体験目標で、現状の実測値や企業導入完了のSLAではない。
前提を満たす未準備ホストで、取得→署名検証→互換性診断→observe→停止を計測する。
取得時間と前提の設定時間を隠さず、初回/再実行、OS/kernel/CPU、エラーと復帰を記録する。
失敗時には変更前の状態を保ち、足りない権限を自動付与しない。

## 3. JSONとサービスの最小設計（W1 / M6）

将来のCLI候補は `--log-format json`。今回追加していない。
stdoutは1行1 JSONに統一し、startup/ready/event/stats/policy操作結果/shutdown/errorを区別。
人向け診断をstdoutへ混在させず、stderrと終了コードも契約にする。
最低限schema_version、event_type、instance/boot識別、scope、mode、monotonic時刻、
family/protocol、接続判断、policy ID/世代、接続結果unknown、欠落と準備状態を扱う。
型・null・未知値・互換性を固定し、u128統計のJSON表現と変換の境界を決める。

argv・environ・payload・秘密値は保存しない。M5スキャンはサービスで既定無効。
PID/IP/commも機微なメタデータになり得るため、最小項目、出力先権限、保持期間を決める。
出力詰まり・broken pipe・容量制限で接続判断と受信loopを無期限に止めない。
既存のqueue/ring/decode欠落をそれぞれ可視化し、JSON整形を接続フックへ持ち込まない。

serviceは既定disabled/observe。指定cgroupの準備と寿命、管理SSHの対象外、二重起動防止、
必要capability、開始失敗、再起動制限、ログ上限、SIGTERM/SIGKILL、停止後の解除を検証する。
BPF object、Map、両familyのlinkが成立する前にreadyを出さない。
今回任意の業務cgroupへattachできる汎用CLIを実装したとは扱わない。

## 4. 配布と信頼（W2）

CLIは現在 `--object` からBPF objectを読む。CLI単体を配っても完成しない。
CLI・対応BPF object・ABI・版・digest・対応表・ライセンス・検証/停止手順を一組にする。
各対応CPU/OS/kernelでその配布物を使い、対象ホストでcargo/Nixなしの試験を通す。
ビルド成功だけでBPF Verifier・BTF/型relocation・attachの互換性は証明できない。

muslは候補であり「Cライブラリ非依存」を約束しない。staticはCコード不使用の意味ではない。
Cargo.lockにはopenssl-sys等もあるため、実際の有効な依存featuresとELFのinterpreter/NEEDEDを検査する。
C依存の存在だけでmusl不可とも決めない。musl/glibcの試作結果、互換性、保守費から選ぶ。
[Cargoの非Rust依存・リンクの説明](https://doc.rust-lang.org/cargo/reference/build-scripts.html)

最初の配布から、同じdigestへSBOM（CLI依存・git固定版・BPF/付属物を含む範囲を明示）、
第三者帰属、ビルド元commit・workflow・toolchain記録、署名と検証手順を結び付ける。
cosignは候補。検証はdigestに加えて想定repo/workflow/refの署名identityとOIDC issuerまで固定し、
改変・別identityの成果物を拒否する。署名は作者の個人的ビルドやコードの安全性を証明しない。
[Sigstoreの検証仕様](https://docs.sigstore.dev/cosign/verifying/verify/)
公開workflowの権限、固定Actions、tag保護、成果物差し替え禁止、release承認と撤回手順も設計する。
今回workflowの作成、外部公開、署名、GitHub Issue/Project操作は行わない。

## 5. 宣言的設定と更新（W3）

TOML/YAML両対応を先に作らず、保守可能な一形式を選ぶ。`sandbox.toml` は流用しない。
初版はliteral IPv4/IPv6+TCP port、重複・容量・schema・scope・modeの厳密検証。
CIDR/DNS/全拒否allowlistは別仕様。現在のdenylistをホワイトリストと呼ばない。
不正設定は旧設定を保持し、modeが明示されなくてもenforceへ移行しない。

初版reloadの候補は明示したローカル管理操作。notifyによる自動監視は必要性が出てから。
ファイルの部分書き込み、所有者/権限、二重更新、世代ID、変更前差分、失敗時復旧を扱う。
既存の単一HashMapへの連続insert/removeは一括反映のatomic性を保証しない。
完全な旧世代または新世代を読む切替方式と、IPv4/IPv6の整合性を選定・検証するまでは、
「無停止・部分反映なし」と呼ばない。保守時間を明示する簡単な方式も比較する。
ユーザー空間の更新、policy reload、BPF program更新は別の操作・試験である。

## 6. metricsの設計候補（W4）

既存 `Snapshot` を共有して接続判断から分離し、scrapeのたびにBPFを再ロードしない。
初期候補はconnect_attempts_total、connect_denied_total、events_emitted_total、
ring_dropped_total、decode_errors_total、queue_dropped_total、ready、last_stats_age_seconds。
名称とschemaは未実装。blocked_packets/scanned_processesは既存統計から生成しない。
per-CPU合算は非原子的、再起動時reset・欠測/未準備・数値変換の限界を明示し、欠測を0にしない。

既定HTTP無効、明示loopback bind、port競合・接続/時間/応答サイズの上限を決める。
9090固定や全interface公開は採用しない。PID・IP・commをlabelにせず、有限のlabelだけにする。
権限を持つloaderにHTTPを同居させる案と既存exporter向け出力を比較し、依存と攻撃面を評価する。
[Prometheusの計測指針](https://prometheus.io/docs/practices/instrumentation/)
JSON/Prometheus形式が出てもDatadog/Splunkへの取り込み成功とはしない。
実collectorでの受信・欠落通知・再起動resetを確認した接続先だけを対応済みとする。

## 7. 継続・中止

各段階の作業と証拠は[タスク一覧](ENTERPRISE_TASKS.md)に一元化する。
まず一人の利用者・一サービス・一環境と、systemd/ホストfirewall/既存NetworkPolicyで足りない理由を確認する。
W1のJSON契約が最初の実装候補だが、今回は着手しない。
具体的な利用価値がなければM5の研究成果として維持し、追加実装を止める選択を残す。
管理経路の遮断、scope拡大、秘密出力、復旧不能、署名/互換性不成立では次段階へ進まない。
全機能追加や版番号1.0を最終ゴールの代用にしない。

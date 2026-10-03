# M5 シークレット警告

状態：完了。専用NixOS VMとPTYで検証済み。今回の研究はM5で区切り、M6は保留。

## 到達点

明示した合成プロセス1件のargvを一度だけ評価し、秘密値なしで警告する。検知は送信の証拠ではなく、自動で拒否ルールを登録しない。

## 実装

- [x] veil-rsの固定コミットの公開API、MITライセンス、検知の制約を確認。
- [x] `connect --scan-argv PID:START_TICKS`で専用cgroupの一つの対象だけにopt-in。
- [x] 読取16 KiB、1プロセス、親待機2秒、子の仮想アドレス空間256 MiB・CPU時間2秒・core無効化。
- [x] 公開Ruleのpattern/validatorから件数だけ集計し、秘密を含むFindingを生成しない。
- [x] CLI/TUIへrule ID・件数・評価状態を表示。argvと値を出力しない。

## 合格条件と確認方法

- [x] 6ルールの偽トークンをRustテストで検知。通常文字列、短い値の見逃し、形式だけ一致する文書例の偽陽性、引数をまたぐ不一致を確認。
- [x] VMで偽GitHub PATを検知し、通常文字列は件数0。CLI・生PTY出力・対象unitのjournal JSON全フィールドに値と引数markerがないことを確認。
- [x] VMで非UTF-8、サイズ超過、対象外、PID終了、開始時刻不一致を未評価に分類。Rustテストで権限エラー・不完全入力・不正な子出力・子失敗・打ち切り後の回収を確認。
- [x] 警告後も合成プロセスのTCP送受信が成功。enforceでも拒否Mapが0件のまま。送信確定や遮断成功と表示しない。
- [x] TUIの終了後、raw/alternate screenとBPFリンクが復帰。M3/M4の実VM回帰を確認。

## 検証記録

基準commitは`3d19235`。検証対象はこのcommitにM5差分を加えた未コミット状態。実際の基準SHAは`git rev-parse HEAD`で確認する。NixOS 26.05、ARM64 Linux 6.18.54、Rust 1.95.0、Aya 0.14.0、aya-ebpf 0.2.1、BPF nightly 2025-12-01 / bpf-linker 0.9.15。veil-core 0.17.0は`83592f5cfb73059f3eaadefcb98bb38262f2ff78`、rustixは1.1.5。既存の依存版は変更せず、追加した推移依存もCargo.lockで固定。

- `scripts/check.sh`：macOSのfmt/Clippy/Rustガード・テスト、Python単体、Markdown、shell、Nix評価を通過。
- `nix build "path:$PWD#checks.aarch64-darwin.architecture" --no-link`：git依存の固定hashを含むNix実ビルド・ガードを通過。
- 構築用Linux VMの`cargo clippy --release --locked --workspace --all-targets -- -D warnings`、`cargo test --release --locked --workspace`を通過。
- `scan::tests::isolated_worker_limits`を別のLinuxテストプロセスで実行し、dumpable無効、parent-death SIGKILL、core/CPU/仮想アドレス空間のsoft/hard limitを確認。
- `scripts/test-secret-vm.sh`：検知・分類・CLI/PTY/journal非露出、通信許可、Map不変、端末とリンクの復帰を通過。結果はGit対象外の`artifacts/m5/acceptance-20261002T225749-51537.json`。
- `scripts/test-policy-vm.sh`と`scripts/test-tui-vm.sh`：拒否・解除・容量・欠落時の判断、操作・resize・終了・起動失敗・描画失敗・SIGKILL時の復帰境界を再確認。
- 固定nixpkgsのcargo-audit：依存334件、RustSec DB commit `117edb3bed98e9be112f277b7615eea3252e7c43`で既知の脆弱性0件、warningなし。未知の脆弱性の不在を証明するものではない。

試験中、systemd-runの既定の説明がargvをjournalへ含めることを確認した。試験起動を固定descriptionと一意なunit名へ修正し、JSON全フィールドを再検査した。初回の失敗は成功扱いにしていない。すべて合成データで、実秘密は使っていない。

構築用VMのdebug検査がディスク不足で失敗したため、既存release成果物を使うLinux検査へ切り替えた。既存ファイル・VMディスク・失敗記録を削除していない。

## 未検証事項と限界

実PID番号の再利用は強制していない。開始時刻不一致の拒否と、procディレクトリFDへの固定を確認した。権限エラーは合成ReadのPermissionDeniedで確認し、VMの実アクセス拒否設定を変更する試験は行っていない。親終了時シグナルの設定を検査したが、読取中の親SIGKILL競合は強制していない。

同一PIDのexec・引数変更・cgroup移動の原子的スナップショット、TLS/payload、environ、初回送信防止、本番DLPは保証しない。読み取り値は子のメモリに一時存在し、完全消去は保証しない。[操作・安全性・既知の見逃し](../../docs/SECRET_WARNING.md)を参照。

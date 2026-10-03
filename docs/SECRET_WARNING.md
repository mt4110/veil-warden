# M5 シークレット警告の仕様と限界

M5は、明示した合成テストプロセスのargvに、既知の秘密形式があるかを一度だけ評価します。通信にその値が含まれるかは判定しません。今回の研究はM5で区切り、M6の常駐化は保留します。

## 操作と対象

専用VM内の`connect`起動時に`--scan-argv PID:START_TICKS`を指定します。省略時はargvを読みません。対象は一つで、専用sliceの子serviceを含みます。開始時刻は`/proc/PID/stat`の第22フィールドです。PIDだけの指定、複数対象、全プロセス走査は受け付けません。

```sh
# 両VM起動・CLIビルド後、合成プロセスによる受入試験を実行
./scripts/test-secret-vm.sh
```

手動操作では、合成プロセスを専用sliceで起動して待機させ、開始時刻を取得します。`systemd-run`を使う場合は`--description="veil-warden synthetic argv fixture"`のような固定説明を必ず指定します。既定の説明は起動コマンド全文をjournalへ記録し得るためです。シェル履歴や別の監査ツールを含む、起動側の記録経路も別に確認してください。VMに転送したビルド済みCLIを次の形で起動します。`1234:56789`は実際の合成プロセスの識別値に置き換えます。

```sh
sudo ./veil-warden connect --object ./veil-warden-ebpf --scan-argv 1234:56789
sudo ./veil-warden connect --object ./veil-warden-ebpf --tui --scan-argv 1234:56789
```

`--enforce`と同時に使っても、検知からルールを追加しません。TUIのb/Enterによる手動拒否は、M3/M4と同じ別の操作です。スキャンはattach前の一度だけで、その後の引数変更・接続イベントでは再スキャンしません。

## 処理と上限

1. 環境を引き継がない子プロセスを一つ起動する。子のstderrは破棄する。
2. argv読み取り前にdumpableを無効化し、core上限0、仮想アドレス空間256 MiB、CPU時間2秒を設定する。設定失敗時は評価しない。
3. hostnameを確認し、対象のprocディレクトリを開く。そのFD経由でstat、cgroup、cmdlineを読む。数値PIDが再利用されても別のprocディレクトリへ読み替えない。
4. statとcgroupを読み取り前後・評価後に確認する。開始時刻不一致や対象外所属は未評価とする。
5. cmdlineは最大16 KiB、stat/cgroupは各4 KiB。上限を1 byte越えて読んで打ち切りを検出する。UTF-8と末尾NULを確認し、引数ごとに照合する。
6. 親は2秒で子を停止・回収する。親終了時の子終了シグナルも設定する。
7. 子は固定状態コードと6個の整数だけを返す。親は状態・件数・出力長を検証し、CLI/TUIへ要約する。

上限は研究用の資源予算です。256 MiBはRSSの実測値ではなく仮想アドレス空間の制限、2秒は性能の保証ではなく打ち切り時間です。起動とスケジューリングの時間があり、終了処理が厳密に2.000秒で完了する保証はありません。スキャン処理はRingBufの受信開始前に行います。

## 実エンジンとの接続

`veil-core` 0.17.0、コミット`83592f5cfb73059f3eaadefcb98bb38262f2ff78`をCargoで固定し、Nixでも取得内容のhashを固定します。未コミットの隣接リポジトリは依存にしません。

公開APIの`get_default_rules()`、`Rule.pattern.find_iter()`、`Rule.validator`を使い、次の6ルールを照合します。APIが秘密値を含む`Finding`を生成する`scan_content()`は呼びません。ソースコメントの`veil:ignore`はargvの抑制指定として解釈しません。ルールが不足した場合は`failed`です。

- `creds.aws.access_key_id`
- `creds.aws.secret_key_config`
- `creds.github.pat.ghp`
- `creds.github.pat.long`
- `creds.slack.token.legacy`
- `creds.key.private_pem`

MITライセンスで利用し、[帰属・ライセンス](third-party/veil-rs-MIT.txt)を保持します。`rustix` 1.1.5の安全なプロセスAPIで資源制限を設定し、このプロジェクトのユーザー空間にunsafeを追加しません。`veil-core`の依存にはHTTP・アーカイブ機能もありますが、M5はそれらのAPIを呼びません。固定依存全体をRustSec監査の対象にします。

## 表示と評価状態

結果は`argv_scan`、検知件数、既知のrule ID、`warning`、`transmission=unknown auto_deny=false`だけです。CLIでは明示したPIDと開始時刻も付けます。argv全文、一致値、環境変数、エンジンの入力由来エラーは出力しません。TUIには起動時の要約を表示します。

`evaluated`はこの時点の読み取れた引数と6ルールの評価完了を意味します。件数0は`no_match`、1以上は`possible_secret`です。安全性や漏洩の確定ではありません。

`unavailable`、`gone`、`identity_changed`、`out_of_scope`、`too_large`、`invalid_utf8`、`incomplete`、`failed`、`timeout`は`unevaluated`です。件数0でも検知なしと表示しません。スキャンが未評価でも監視は継続し、失敗を自動拒否へ変換しません。

## なぜこの範囲に絞るか

専用VMとcgroupはホストやSSHを巻き込む操作ミスを減らします。対象指定・読取上限・子の資源制限は収集範囲と負荷を限定します。数値と固定IDだけの結果型、子のstderr破棄、dumpable無効化はログやクラッシュダンプへの値の露出経路を減らします。スキャナはpolicy ControllerやMapを所有せず、検知は通信判断に接続しません。

完全な防御境界ではありません。VMやVerifierにも不具合の可能性があり、root管理者に対する認可境界ではありません。読み取ったargvは子のメモリに一時的に存在し、物理メモリの消去や管理者からの秘匿は保証しません。実秘密を使わず、合成データで検証します。

PID開始時刻とFDは再利用誤読を減らしますが、同一PIDのexec、引数変更、所属の移動を含む状態の原子的スナップショットを保証しません。対象が協調して待機する合成試験を前提とします。

形式が一致する架空トークンや文書例も検知されます。短いトークン、新形式、分割した値、stdin・ファイル・メモリ・環境変数の値は見逃します。TLSを復号せず、argvに秘密があることから送信を推定しません。初回送信防止、既存通信の停止、本番DLPは対象外です。

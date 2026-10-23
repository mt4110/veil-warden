# サービス同定の検証記録：2026-10-23

ArenaのARM64 Linux隔離VM1台で、Wardenの`diagnose --service`を確認しました。
同定・終端証拠・controllerによる停止は成功しました。BPF attachや通信保護を認定する試験ではありません。

## 入力と再現範囲

- Warden main `3a72816`にローカルのサービス同定実装を追加したsnapshotをNixビルド。
- CLI/eBPFの一体パッケージは改変せず、manifestのhashで確認。
- Mac ARM64／QEMU TCG、NixOS aarch64-linux、2 vCPU/4 GiB、NICなし。
- 診断はゲスト専用UID501、CAP_SYS_PTRACEのみ、BPF拒否・AF_UNIX制限下で実行。
- 今回のPRで新しくVMを起動した結果ではなく、保存済みの実試験証拠を整理したもの。

[公開集計](validation/service-identity-20261023.json)に、実装3ファイル・パッケージ・証拠のSHA256を記録しています。
今回のPR内のmain.rs、diagnose.rs、service_identity.rsは、実VMで使ったビルドsnapshotとbyte一致を確認しました。
実VM実行時のソース状態、PRのcommit、CIでのビルドは分けて扱います。
rawログ、旧失敗証拠、VMイメージはローカルに保持し、このリポジトリに同梱しません。

## 結果

| 入力 | 終了コード | 結果 |
| --- | --- | --- |
| 合成サービス | 1 | service_identity=candidate、PID/cgroup/InvocationIDを取得 |
| 不在サービス | 1 | service_identity=unknown |
| SSH対象 | 2 | 入力段階で拒否、診断stdoutなし |
| service/cgroup混在 | 2 | 入力段階で拒否、診断stdoutなし |

合成サービスはarena-fixture.serviceです。取得前後でPID起動tick・cgroup inode・起動世代等の一致を確認する実装です。
通常サービスの終了1は既存loaderの専用sandbox制限に合わないためです。
同定がcandidateでも、loader_scope=blocked、runtime_acceptance=unknown、deployment_ready=falseを維持します。
全4ケースの診断と子プロセスのtraceでBPF syscallは0、socket familyはAF_UNIXだけでした。
OS全体の全プロセスを追跡したという意味ではありません。

## 終端処理と停止

Arena側では初回試験で証拠出力と終端取得が失敗しました。UID・マーカー・ログを修正した再試験が今回の成功記録です。
Wardenの同定実装はその修正前後で変更していません。

- 実VMで書き込みUIDと出力先所有者UIDが501で一致し、終端マーカーも一致。
- controllerはservice_completedを検出し、証拠同期後にSIGTERMでQEMUを停止・回収。
- 強制SIGKILLなし。停止後にcontrollerとQEMUの両PID不在を独立に確認。
- ゲストOSの自発的なpoweroff完了を確認した結果ではない。
- 試験VMと専用builderは停止し、旧bundle・失敗証拠を保全。

詳細は[Arenaの検証記録](https://github.com/mt4110/veil-arena/blob/main/docs/WARDEN_NONLOADING_VALIDATION.md)と
[公開集計](https://github.com/mt4110/veil-arena/blob/main/docs/validation/warden-nonloading-20261023.json)を参照してください。
Arena側の入口はPR #35でマージ済みです。新しい検証には、この同定実装を含むWardenソースを指定する必要があります。

## ローカル回帰と未確認事項

型付きpropertyの不正値、保護対象・入力の拒否、PID再利用・起動世代変更・cgroup入れ替え・boot変更をロジック上で検査しています。
空のcgroupパスを参照しない回帰と、専用モジュールのprocfs読取・非変更APIのアーキテクチャ検査を追加しました。
これらはLinuxの再起動レースやnamespace不一致を実VMに注入した証拠ではありません。

x86_64での今回の同定実行、一般企業の実サービス、クラウド、user manager、コンテナ同定、監視・拒否・解除は未確認です。
読み取り不能はunknownとし、自動昇格しません。CAP付与や本番設定変更の手順として、この検証VMの設定を流用しないでください。

# サービス同定：非ロード診断

状態：ローカル回帰と、ArenaのARM64隔離VM1台でのサービス同定・終端証拠・停止を確認済みです。
[検証記録](SERVICE_IDENTITY_VALIDATION.md)に入力と限界を記載しています。BPFロード・実サービスの保護は未検証です。
既存の`--cgroup`診断と専用VM loaderの制限を維持します。

```sh
veil-warden diagnose --service example.service --lang ja
veil-warden diagnose --service example.service --lang en
veil-warden diagnose --help
veil-warden diagnose -h
```

`--service`と`--cgroup`は排他的です。system managerの既にロードされた通常のserviceだけを対象にします。
サービスをロード・起動・停止せず、設定やcgroupを書き換えません。対象のargv、環境変数、unitのExecStartや秘密ファイルは取得しません。

## 取得と照合

1. 固定位置の`busctl`でsystemdの`GetUnit`を呼び、unit objectを取得します。`LoadUnit`は使いません。
2. 読み取りpropertyから正規名、loaded/active/running、InvocationID、Type、MainPID、ControlGroup、Delegateを取得します。
3. `/proc/PID/stat`の起動tickとPID、`/proc/PID/cgroup`の完全一致、非symlinkのcgroupディレクトリのdevice/inode、boot IDを確認します。
4. PID・mount・cgroup namespaceのリンクを自身・対象PID・PID 1の間で照合します。自身を含むサービスは対象外です。
5. propertyとプロセス情報を再取得し、起動世代・PID・cgroup inode等の変化を検出したら未確認に戻します。

これは取得前後の比較であり、原子的なスナップショットや将来の同一性を保証しません。
将来のattach処理では、FDの所有と同定結果の再検証を別に実装する必要があります。

## 制限と出力

- 入力名は最大255 bytes、ASCIIの限定文字と`.service`。未指定テンプレート、SSH、Warden自身、Docker/containerd/Podmanの管理デーモンは除外します。別名でも正規名を再検証します。
- Typeはsimple/exec/notify/forking、MainPIDは2以上、Delegate=false、cgroup末尾は正規unit名との一致が必要です。コンテナや委譲先、user manager、既存ソケットの保護を認定しません。
- `busctl`は`/usr/bin/busctl`または`/run/current-system/sw/bin/busctl`が必要です。PATHの任意の実行ファイルを選びません。依存は同梱せず、不在なら未確認です。
- D-BusへのローカルUNIX IPCを使います。既存の`--cgroup`の「ネットワーク接続なし」と異なる契約です。外部宛TCP/UDPやBPF syscallを使う処理は追加しません。
- D-Busの自動activationを無効にし、各bus呼び出しは1秒、全bus取得の共有期限は8秒、標準出力は64 KiBで制限します。通常のローカルprocfs/sysfs読み取りを前提とします。
- 管理者への昇格や再試行は行いません。namespaceリンク等を読めない一般ユーザー、非Linux、非systemd、不在・停止中のunit、PID不一致、取得中の再起動は`service_identity=unknown`です。
- 十分に一致すれば`service_identity=candidate`と正規unit、MainPID、ControlGroup、InvocationIDを出します。プロセスの全子孫や通信の網羅を保証しません。

どちらも`apply_supported=false`、`deployment_ready=false`を維持します。
通常サービスでは既存loaderの対象外なので終了コードは1です。同定がcandidateでも全体の合格ではありません。
入力不正は2、ヘルプは0、その他は既存の診断契約に従います。

## 検証と次のゲート

Mac上では型付きpropertyの解析、入力拒否、PID起動tick、再起動・PID再利用・cgroup交換・boot変更の比較、実CLIの非Linux結果を試験します。
これらの異常系はフィクスチャによるロジック検証です。実VMで再起動やnamespace不一致を故意に注入した結果ではありません。

2026-10-23、Arenaの`warden-nonloading-v1`で、合成serviceの同定と不在・保護対象・混在入力を確認しました。
診断と子プロセスのtraceでBPF syscallは0、socket familyはAF_UNIXのみでした。
証拠同期後にcontrollerがQEMUを停止・回収し、ゲスト自身のpoweroff完了とは区別しています。
使用したWarden実装はPR化前のローカルsnapshotです。今回の実装3ファイルとのhash一致を[公開集計](validation/service-identity-20261023.json)に記録します。

対応するArena入口は[veil-arenaの非ロード試験](https://github.com/mt4110/veil-arena/blob/main/docs/WARDEN_NONLOADING.md)を参照してください。
一般ユーザーへのCAP付与を自動で行う機能ではありません。実サーバーの権限変更・VM起動・BPFロードは別の作業です。

一次資料：systemdの[GetUnitとLoadUnitの違い、Unit/Service property](https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.systemd1.html)、
[busctlのget-property、timeout、auto-start](https://www.freedesktop.org/software/systemd/man/latest/busctl.html)。
実装時に参照した文書はsystemd 262です。以前のsystemdでpropertyが欠ける場合も未確認にします。特定バージョンの動作認定ではありません。

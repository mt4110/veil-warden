# veil-warden VM 検証

M0 は `warden-preflight` で環境と通信範囲を実測します。構築・起動・停止の手順は [実行ガイド](../../docs/GUIDE.md) を参照してください。

## M0 の合格条件

- ARM64 Linux、cgroup v2、BTF が利用可能。
- cgroup_skb、cgroup_sock_addr、RingBuf、HashMap、per-CPU Array の feature probe が成功。
- SSH は専用 slice の外にあり、合成クライアントは slice に入ってから socket を作る。
- IPv4/IPv6 loopback の受信サーバーが固定した合成文字列を受信。
- 同じイメージで再起動したとき、ゲストの一時変更が消え、同じ試験が成功。

BPF link の列挙ができることは確認しますが、自作プログラムの cgroup attach は M1 の実試験です。既存の kernel/system service が作った BPF link と自作の link を混同しません。

## 記録

ローカルの `artifacts/m0` に Nix 設定の版、preflight の JSON、boot ID と slice の資源制限を記録します。Git には非機密の結果概要を [M0](../../milestones/00-sandbox/README.md) に残します。

VM 以外の環境で root として実行しないでください。preflight は root の feature probe と専用 systemd unit を使います。実秘密・外部 IP・payload を試験へ追加しません。

## M1 の受け入れ試験

`./scripts/build-counter-vm.sh` の後に `./scripts/test-counter-vm.sh` を実行します。`cases/packet_counter.py` は実際のAya CLIを起動し、固定合成UDPのIPv4/IPv6受信、counter増加、対象外隔離、終了後のlink消失を検証します。CLIのroot scope指定・ロード失敗・二重起動も拒否される必要があります。

通常終了・SIGTERM・SIGKILLは別々に実attachして調べます。systemd自身のprogram IDは再生成で変わり得るため、ID以外の全attachment fieldと自作linkの消失を確認します。ローカルJSONは `artifacts/m1`、共有できる概要は [M1](../../milestones/01-packet-counter/README.md) に記載します。

## M2 の受け入れ試験

共通ビルド後に `./scripts/test-connect-vm.sh` を実行します。`cases/connect_monitor.py` はIPv4/IPv6成功と拒否TCP、対象外TCP、connected UDP、PID/TID/cgroup・宛先の一致を検証します。診断用の受信遅延でRingBufを満杯にし、欠落・最終計数・短命プロセスの名前不明を確認します。

通常終了・SIGTERM・SIGKILLの2リンク解除に加え、connect6を欠くfixtureの部分起動失敗も試します。ローカルJSONは `artifacts/m2`、概要は [M2](../../milestones/02-connect-monitor/README.md) に記載します。

## M3 接続制御

ホストから `./scripts/test-policy-vm.sh` を実行します。共通ビルド成果物と `connect_policy.py` / `connect_monitor.py` を専用VMへ転送し、rootでloopback合成TCP試験を行います。M3の両フックは専用sliceだけへattachし、管理SSHと既存systemd BPFは保持します。root以外の制御拒否、容量超過、既存接続、mapped IPv6、ログ欠落と終了後復帰も確認します。JSONは `artifacts/m3` に保存します。

## M4 TUI

`./scripts/test-tui-vm.sh` はLinuxテストバイナリと通常CLIを専用VMへ転送し、PTYを使って実キー操作・リサイズ・更新失敗・端末復元・リンク解除を確認します。描画失敗fixtureはPTY内で `--ignored --exact` により明示実行し、1件実行されたことも確認します。JSONと画面のテキストsnapshotをartifacts/m4に残します。ANSI解析は試験で使うsubsetに限定し、一般的なterminal emulatorとして提供しません。

## M5 argv警告

M5は`--scan-argv PID:START_TICKS`で指定した専用cgroupの合成プロセス1件を、attach前に一度だけ評価します。既定ではargvを読みません。値を含むFindingを生成せず、rule ID・件数・評価状態だけを返します。検知からルールを追加せず、未評価と検知なしを区別します。資源上限、PID/FDの確認、既知の見逃しと復帰手順は[仕様と限界](../../docs/SECRET_WARNING.md)を参照してください。

両VMとビルド成果物を準備して`./scripts/test-secret-vm.sh`を実行します。合成値のCLI・PTY・対象unitのjournalへの非露出、通常入力との差、開始時刻不一致・終了・対象外・非UTF-8・サイズ超過、通信許可とMap不変を確認します。権限エラーと子の打ち切り・失敗はRustテストで確認します。

# M2 接続監視

状態：完了（2026-10-02、実VMで検証済み）。前提：M1。

## 到達点

専用cgroupのTCP IPv4/IPv6接続試行を、同じRust CLIの `connect` モードで観測する。通信は常に許可し、接続成功は判定しない。

## 実装

- [x] 56 byte、alignment 8のConnectEvent ABI v1を固定し、全fieldのoffsetを両コンパイラで検査。
- [x] connect4/connect6をFD linkでattach。片方の起動失敗時も取得済みlinkを解放。
- [x] 16 KiB RingBufをTokio AsyncFdで受信。予約失敗をkernelのper-CPU STATSで計数。
- [x] 長さ・版・family・hook・protocol・action・reserved等を検証し、デコード失敗を別計数。
- [x] PID/TID・宛先・cgroup ID・時刻を表示。commは最大64 byte読んで制御文字を除去し、最大32文字に制限。
- [x] comm読み取りを表示workerに分離。表示待ち128件を超えたイベントをqueue_droppedとして計数。

実装先はcommon/src/lib.rs、ebpf/connect.rs、user/{decode.rs,events.rs,process.rs,output.rs}です。ユーザー空間にunsafeキャストやPod実装を追加していません。ABIの整数はbpfelに合わせてlittle endian、IPはnetwork octet、portはkernelで一度だけhost-valued整数へ変換します。

## 合格条件と結果

- [x] IPv4 `127.0.0.1` とIPv6 `::1` のIP/port、TGID/TID、cgroup IDが期待値と一致。
- [x] worker threadからの接続でPIDとTIDが異なることを確認。
- [x] 合成bytesで短い/長い長さ、未知版、不正値、byte order、非自明なIPv6アドレスを検査。
- [x] 対象外TCPおよびconnected UDPはイベント・attemptedに含めない。
- [x] 拒否されたTCP接続も接続試行として表示し、connection_result=unknownを維持。
- [x] 受信遅延と高頻度イベントでRingBuf欠落が増え、固定容量と計数を確認。
- [x] 短命プロセスのcomm取得失敗でも受信・表示を継続。
- [x] 通常終了・SIGTERM・SIGKILL後に自作の2リンクが残らず、管理SSHを維持。
- [x] connect6を欠いた専用fixtureで部分起動失敗とリンク解除を確認。
- [x] 同時起動を拒否。M1のパケットカウンタ回帰試験も成功。

通常試験はIPv4成功1件、IPv6成功1件、IPv4接続拒否1件の合計3イベントです。通常終了とSIGTERMの最終statsはattempted/emitted/decoded/displayedがすべて3、ring_dropped/decode_errors/queue_droppedがすべて0でした。SIGKILLでも3イベントとリンク解除を確認し、最終statsは出力されません。

## 欠落・メモリの試験

ローカルの閉じたTCP portへ4,096回ずつ2回、計8,192回の合成接続試行を行い、診断用reader delayを800 msにしました。外部通信や実トークンは使いません。

| 指標 | 実測 |
| --- | ---: |
| attempted | 8,192 |
| emitted / decoded / displayed | 255 / 255 / 255 |
| ring_dropped | 7,937 |
| decode_errors / queue_dropped | 0 / 0 |
| RSS（前→後） | 25,056→25,060 kB |

`attempted = emitted + ring_dropped`、`decoded = displayed + queue_dropped` を終了後に確認しました。欠落件数は入力とスケジューリングで変わり得るため、この数値自体を合格基準にはしません。表示キュー満杯・Closedの挙動はLinux Rustテストで別途確認しました。

メモリ増加を抑える根拠は固定RingBuf・有界queue・単一worker・comm読取上限・履歴/キャッシュなしです。上記の短時間RSS測定は長時間のリーク不存在や性能保証ではありません。

## 固定版と検証環境

M1のNixOS 26.05、kernel 6.18.54（aarch64）、Rust stable 1.95.0、nightly-2025-12-01、Aya 0.14.0 / aya-ebpf 0.2.1、bpf-linker 0.9.15を保持。追加のTokioは1.53.1をCargo.tomlとCargo.lockで固定しました。基準commitはM1の `0f582a4`、M2変更は未コミットの作業ツリーです。

Rust guard 11件、CLI/decoder/processテスト6件（macOS）および7件（Linux）が成功。Python公開鍵境界、Markdown lint、fmt、Clippy、ShellCheck、Nixfmt、Nix flake checkを確認しています。cargo-audit 0.22.1はRustSec DB `117edb3bed98e9be112f277b7615eea3252e7c43` に対して既知脆弱性・警告とも0件。全67packageのlicense宣言を確認済みです。Tokioと追加の推移依存はMIT等の許諾を選択できます。第三者noticeの配布確認はrelease時に別途必要です。

最終VM記録は `artifacts/m2/acceptance-20261002T202222-96386.json`、Linux記録は `final-linux-checks.log`、M1回帰記録は `m1-regression.txt` に保持。実行方法は [ガイド](../../docs/GUIDE.md#m2-接続監視の実行) を参照してください。GitHub remote CIは未実行です。

## 限界と対象外

argv/environ/payloadは読まない。commはPID再利用・終了との競合があるbest effort情報で、イベント時点の名前の証明ではない。取得失敗はunknownと状態を表示する。

TCPの接続試行を表示するだけで、TLS復号、通信の安全判定、接続成功の追跡、通信拒否は実装していません。UDP/QUIC、既存socket、IPv4-mapped IPv6の実カーネル挙動、長時間・高負荷での性能は未検証です。M3では両familyとルールの境界を別途検証します。

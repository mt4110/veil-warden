# M2 接続監視

状態：未着手。前提：M1。

## 到達点

TCP の IPv4/IPv6 接続試行を同一 CLI で観測する。

## 実装先の予定

common/src/lib.rs、ebpf/connect.rs、user/{events.rs,process.rs,output.rs}、tests/fixtures。詳細は [ディレクトリ構成](../../docs/DIRECTORY_LAYOUT.md) を参照。

## 作業

- [ ] 56 byte の ConnectEvent 提案を実装前に固定し、ABI 検査を置く。
- [ ] connect4 と connect6 を attach し、観測のみで許可する。
- [ ] RingBuf を非同期で受信し、予約失敗とデコード失敗を数える。
- [ ] PID/TID、宛先、取得できた comm を表示し、名前不明を区別する。

## 合格条件

- [ ] IPv4 loopback と IPv6 loopback の IP/port が期待値と一致する。
- [ ] 合成イベントで受信長不足、未知版、不正値、エンディアンを検証する。
- [ ] 受信遅延・高頻度イベントで欠落数が増え、無制限のメモリ増加がない。
- [ ] 短命プロセスと comm 読み取り失敗でも受信処理を継続する。
- [ ] CLI が接続試行と実際の接続成功を区別する。

## 中止と対象外

IPv6 またはイベント ABI が未検証なら M3 を開始しない。argv/environ は読まない。

## 検証記録

未実施。実行した環境、入力、結果、未検証事項だけを追記する。

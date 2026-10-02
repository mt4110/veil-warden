# M3 接続拒否と解除

状態：完了（2026-10-02、専用NixOS VM実測）。前提：M2。

## 到達点

事前ルールで対象 cgroup の TCP 新規接続を拒否できる。

## 実装先

kernelのconnect.rs、commonのキー・verdict、userのcli.rs/policy.rs/events.rs、tests/vm/cases/connect_policy.py。詳細は [ディレクトリ構成](../../docs/DIRECTORY_LAYOUT.md) を参照。

## 作業

- [x] family・IP・port・protocol の tuple でルールを作る。
- [x] observe を既定にし、enforce でのみ Map に従い拒否する。
- [x] 一つのルールの追加/解除と、更新成功・失敗・現状の表示を実装する。
- [x] 両 family の準備完了前にテストクライアントを起動しない。

## 合格条件

- [x] 拒否ルールを接続前に登録し、IPv4/IPv6 の新規接続が拒否され、サーバーに届かない。
- [x] ルール解除で新規接続が成功する。未登録 port と対象外クライアントは影響を受けない。
- [x] 既存接続は停止しないことを確認し、制約として表示する。
- [x] Map 容量不足/更新失敗は成功扱いにならない。
- [x] 一方の attach 失敗、通常終了、SIGTERM、SIGKILL 後にリンクが残らず接続が復帰する。
- [x] クライアント errno、サーバー受信、SSH 維持を記録する。

## 中止と対象外

対象外への影響や解除失敗があれば遮断機能を完成扱いにしない。プロセス kill と firewall バイパスは追加しない。

## 検証記録

実行環境：Apple Silicon Mac上の独立NixOS VM、aarch64、Linux 6.18.54、2 CPU / 4 GiB。BPFはnightly-2025-12-01とbpf-linker 0.9.15、CLIはRust 1.95.0、Aya 0.14.0 / aya-ebpf 0.2.1。既存のflake/lockで固定し、miseは使わない。

`./scripts/build-counter-vm.sh` と `./scripts/test-policy-vm.sh` を実行した。対象sliceに所属してから新しいsocketを作り、実サーバーの受信とクライアントerrnoを確認した。

| 検証 | 実測結果 |
| --- | --- |
| 既定observe | 全許可、ルール追加は非ゼロ終了、Mapはゼロ件のまま |
| IPv4 / IPv6拒否 | EPERM（errno 1）、拒否フェーズのサーバー受信データはゼロ |
| IPv4-mapped IPv6 | IPv4の同じルールIDで拒否 |
| 未登録IP・port、UDP、対象外cgroup | 成功、対象外とUDPのconnectイベントは収集しない |
| 解除 | 両familyで新規TCP接続が成功 |
| 重複追加・不存在解除 | 非ゼロ終了、現在のMap状態を表示 |
| 容量超過 | 16件保持、17件目はbpf_map_update_elemがE2BIG（errno 7）、既存ルール不変。解除後の追加は成功 |
| 既存接続 | 追加前・追加後・容量試験後の3回の送受信が成功 |
| 部分起動失敗 | connect6欠落fixtureで失敗、connect4のlinkも解除、接続復帰 |
| 通常終了 / SIGTERM / SIGKILL | 両linkと制御endpointが残らず、IPv4/IPv6新規接続が復帰。既存systemd BPFと管理SSHを維持 |
| 制御要求 | 非rootクライアントと偽サーバーを拒否、不正UTF-8・257 byte要求はエラー、未完了要求はtimeout。監視継続 |
| ログ欠落 | 4,096試行すべてEPERM、denied=4,096、emitted=255、ring_dropped=3,841、受信ゼロ |

負荷試験ではattempted=emitted+ring_dropped、decode_errors=0を確認した。これらは一回の合成loopback実測で、通信速度、長時間安定性、実ネットワーク上の網羅性は測定していない。M1/M2の実VM回帰試験、macOS/LinuxのRustテスト・Clippy、Markdownlint・shellcheck・Nix検査も通過した。

表示workerが待機中もstdoutを保持するとreadyのflushと終了が止まる競合を再検証で発見し、表示1件の間だけstdoutをロックするよう修正した。正常終了と制御応答を実VMで再検証した。失敗した試験の成果物も自動削除しない。

ABIはactionとpolicy IDの意味を拡張したためv2へ更新。ルールは最大16件、永続化なし、操作は単一キーに限る。更新と同時進行のconnectの厳密な順序、既存・受け渡しsocket、UDP/QUIC、TLS復号、秘密の流出防止、常駐サービスは保証対象外。停止後は保護が解除される研究用fail-open方針。

操作手順は [実行ガイド](../../docs/GUIDE.md#m3-接続拒否と解除の実行)、影響範囲と根拠は [安全性文書](../../docs/SAFETY.md#m3-の事前拒否と復帰) を参照。

ローカル受け入れ記録： `artifacts/m3/acceptance-20261002T213547-16592.json`（Git対象外、実行ごとに日時・PIDが変わる）。

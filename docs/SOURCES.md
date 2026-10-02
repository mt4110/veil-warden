# veil-warden 一次資料と設計上の訂正

2026年10月2日に公式文書・公式ソースを参照した設計。VM と依存バージョンは未選定であり、M0/M1 で採用版の仕様と実動作を再確認する。master/latest の情報は固定版の検証を代替しない。

## 確認した資料

- [Aya 開発環境](https://aya-rs.dev/book/start/development.html)：Rust toolchain と bpf-linker の準備。
- [Aya プログラム制約](https://aya-rs.dev/book/)：no_std、heap 不在等の制約。
- [Aya 公式テンプレート README](https://github.com/aya-rs/aya-template/blob/main/README.md)：Cargo build script による eBPF ビルド連携。xtask 必須とはしない。
- [Aya 0.13.1 RingBuf](https://docs.rs/aya/0.13.1/aya/maps/ring_buf/struct.RingBuf.html)：RingBuf と AsyncFd の利用、Linux 5.8 以上。0.13.1 は確認例であり採用版の宣言ではない。
- [Linux BPF ring buffer](https://docs.kernel.org/bpf/ringbuf.html)：複数 CPU で共有するキューと、予約失敗時の扱い。
- [Linux Verifier](https://docs.kernel.org/bpf/verifier.html)：メモリアクセスや初期化状態の検査。
- [Linux cgroup BPF 実装](https://github.com/torvalds/linux/blob/master/kernel/bpf/cgroup.c)：sock_addr フックの拒否と EPERM、cgroup link の管理。

## 貼付案から修正した点

| 元の主張 | 設計での扱い |
| --- | --- |
| VM と Verifier があるので 100%安全 | 絶対保証を撤回。設定ミス、共有ファイル、資源負荷、ソフトウェアの欠陥が残るため、限定対象と復帰検証を設ける |
| connect4 で全通信と payload を監視 | connect は接続試行のフック。payload、未接続 UDP、既存接続の各送信を網羅しない |
| IPv4/IPv6 は一般に別フック | connect4/connect6 は分かれるが、XDP/TC 等の全フックが IP family ごとに分かれるわけではない |
| PID を100%正しく特定 | 呼出し時の TGID/TID と、実際のソケット所有者・アプリ帰属を区別する |
| return 0 は ECONNREFUSED | 参照した sock_addr 実装の既定拒否は EPERM。VM の client errno を確認する |
| イベント後のスキャンで初回漏洩防止 | return 1 後は接続処理が進む。後から Map を更新しても初回送信や既存接続を止める保証はない |
| argv/environ で TLS を含む全秘密を検知 | メモリ、ファイル、stdin、動的生成などは対象外。秘密の存在と送信の事実も別 |
| IP 自動ブロックで安全 | 共有 IP の無関係な接続へ波及する。M3 は明示した対象 cgroup と宛先 tuple の事前ルール |
| コマンド全文をログへ出す | 秘密をログへ二次流出させるため禁止。値を含まない状態・件数だけ出力 |
| PerfEventArray が RingBuf | 別の Map。週末案は RingBuf に統一し、AsyncPerfEventArray の例と混ぜない |
| repr(C) と padding で自動的に安全 | 全 byte の初期化とサイズ/offset/受信長/版の検証が必要。Verifier 拒否は kernel panic とは別 |
| RingBuf/非同期で取りこぼしゼロ、CPUゼロ、遅延ゼロ | 保証しない。欠落カウンタを実装し、必要時に負荷と条件を記録して測定する |
| 通常 firewall の拒否を return 1 で強制突破 | このフックの許可は後段の firewall を含む全経路の許可ではない。バイパス機能は設計対象外 |
| infinite memlock と root が常駐の必須条件 | 採用 kernel、memcg、必要 capability と制限を実環境で確認する |

表の初回送信、秘密の評価範囲、VM 運用に関する内容は、フックのタイミングと提示コードからの設計評価。完全な DLP の実証結果ではない。veil-rs のソース/API、NixOS/VM 基盤の適合性は今回未確認。

## M0 の環境に用いた一次資料

- [固定 nixpkgs の QEMU VM モジュール](https://github.com/NixOS/nixpkgs/blob/4feb8eb8bf30f323a8a5d285f14ee51d6a7197b1/nixos/modules/virtualisation/qemu-vm.nix)：store image、一時 root、host 側 QEMU、転送・共有設定。
- [固定 nixpkgs の bootstrap builder](https://github.com/NixOS/nixpkgs/blob/4feb8eb8bf30f323a8a5d285f14ee51d6a7197b1/nixos/modules/profiles/nix-builder-vm.nix)：公開ホスト鍵、bootstrap の構成。既定の全インターフェース転送はそのまま使わず loopback へ変更。
- [NixOS 公式マニュアル](https://nixos.org/manual/nixos/stable/)：宣言的設定と VM の扱い。実装の版判断は上記の固定ソースを優先。

構築用 VM と専用 VM を実際に起動し、機能確認と IPv4/IPv6 の合成通信・復帰を確認しています。M1の採用版は config/m1-toolchain.toml、Cargo.lock、flake.lockで固定しています。

## M1 採用版の一次資料

- [Aya 0.14.0 CgroupSkb実装](https://github.com/aya-rs/aya/blob/aya-v0.14.0/aya/src/programs/cgroup_skb.rs)：load、attach、FD linkへの変換と終了時の寿命。
- [Aya 0.14.0 linkとattach mode](https://github.com/aya-rs/aya/blob/aya-v0.14.0/aya/src/programs/links.rs)：Ayaのflags=0を使い、LinuxのBPF_LINK_CREATEが内部でALLOW_MULTIを追加する挙動を確認。
- [aya-ebpf 0.2.1 PerCpuArray](https://github.com/aya-rs/aya/blob/aya-ebpf-v0.2.1/ebpf/aya-ebpf/src/maps/per_cpu_array.rs)：固定長counterのMap参照。
- [bpf-linker 0.9.15](https://github.com/aya-rs/bpf-linker/blob/v0.9.15/Cargo.toml)：LLVM 21の対応を確認し、LLVM 21のnightlyを選択。
- [固定Aya template](https://github.com/aya-rs/aya-template/tree/c0fd79891b5ac8f73c1092bc2a374268d0548c7c)：依存版・workspaceの参考。generatorは実行せず最小構成を作成。
- [Linux 6.18 cgroup BPF](https://github.com/torvalds/linux/blob/v6.18/kernel/bpf/cgroup.c)：cgroup_skb送信時の戻り値1は許可。

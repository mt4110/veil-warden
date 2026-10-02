# M0 サンドボックス

状態：完了。2026年10月2日、Apple Silicon Mac 上で構築用 VM と専用 NixOS VM を起動し、機能・範囲・復帰を確認しました。

## 到達点

再現できる ARM64 Linux VM、限定した合成プロセス、管理用接続、検証基盤を用意しました。M1 の自作 eBPF プログラムは未実装です。

## 作業と合格条件

- [x] NixOS/kernel/Rust/検証ツールを flake.lock の revision・hash で固定。
- [x] ホストの権限設定を変更せず、ローカルの構築用 VM で Linux イメージを作成。
- [x] 専用 VM の共有を公開鍵ディレクトリだけに限定。秘密鍵はホストの状態ディレクトリに保持。
- [x] cgroup v2、BTF、cgroup_skb/cgroup_sock_addr、RingBuf、HashMap、per-CPU Array の feature probe が成功。
- [x] BPF link を列挙できることを確認。自作 cgroup link の attach は M1 で実施。
- [x] SSH がテスト slice の外にあり、クライアントが所属後にソケットを作ることを確認。
- [x] IPv4/IPv6 loopback サーバーが合成文字列を受信。
- [x] 同じ固定イメージの再起動後に一時的な marker が消え、preflight が再度成功。

## 固定した版

| 対象 | 版 |
| --- | --- |
| nixpkgs | 4feb8eb8bf30f323a8a5d285f14ee51d6a7197b1 |
| NixOS | 26.05.20261001.4feb8eb |
| Linux | 6.18.54 |
| Rust stable | 1.95.0 |
| Nix shell 内の Nix | 2.34.8+1 |
| bpftools | 6.18.7 |
| Python | 3.13.15 |
| QEMU | 10.2.4 |
| markdownlint-cli2 | 0.21.0 |

macOS 側の既存 Nix daemon は 2.33.1 のままで、設定・権限を変更していません。nightly/Aya/bpf-linker は M1 で採用版を固定します。

## 実測した範囲と資源

管理用 SSH は `/system.slice/sshd.service`、合成クライアントは `/warden.slice/warden-test.slice/warden-m0-probe.service` に所属していました。

各 VM は2 CPU・最大4 GiB。テスト slice は CPU 100%、MemoryMax 512 MiB、TasksMax 64。専用 VM の root は一時的なメモリ領域で、ディスク削除をせず復帰しました。

最初の構築用ディスク16 GiBは再ビルドで満杯になったため、既存状態を保持して新しい32 GiBディスクで最終版を構築しました。古いディスクや検証結果を削除していません。

## 静的検証

- Markdown リント、Rust 整形、Clippy、ShellCheck、Nix 整形・flake 評価が成功。
- Rust のアーキテクチャガード9件が macOS と ARM64 Linux で成功。
- 公開鍵境界の Python 試験2件が成功。
- macOS の Nix flake checks のビルドが成功。

ガードのうち、将来の kernel/common/loader と workspace 登録を確認する条件は、該当ソースがまだ存在しないため実コードの評価は保留です。危険な設定・依存・loader 宣言を拒否する負例は試験しています。テスト数をランタイム品質の証明にしません。

GitHub Actions の設定は用意しましたが、リモートでの実行は未確認。CI の成功とは呼びません。

## 証拠と再実行

ローカル `artifacts/m0` に preflight、再起動後の結果、版と slice の実設定を保存しています。手順は [実行ガイド](../../docs/GUIDE.md)、理由と限界は [安全性文書](../../docs/SAFETY.md) を参照してください。

M1 では verifier による実ロード、自作プログラムの cgroup attach、終了後の link 解除を確認します。パケットカウンタ、接続監視、拒否・解除、秘密検知、常駐化は未実装です。

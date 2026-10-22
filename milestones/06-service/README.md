# M6 サービス化

状態：保留（未着手）。今回の研究はM5で区切り、常駐化は別の判断があるまで開始しない。前提：M3。M5 は必須ではない。

## 到達点

VM の限定対象に observe デーモンを起動・停止できる。

## 実装先の予定

`infra/nixos/service.nix`（候補）、`crates/veil-warden/src/output.rs` と起動処理。詳細は [ディレクトリ構成](../../docs/DIRECTORY_LAYOUT.md) を参照。

## 作業

- [ ] --daemon では TUI/TTY を使わず、非機密の構造化イベントを出す。
- [ ] Nix store 内のビルド成果物を ExecStart に使い、初期無効/observe にする。
- [ ] 採用 kernel の必要 capability、memlock/memcg、cgroup 作成と所有を確認する。
- [ ] 再起動回数の制限、SIGTERM、リンク寿命、ルールの非永続性を定義する。

## 合格条件

- [ ] サービス開始/停止と VM 再起動後の状態、対象 cgroup の準備順を確認する。
- [ ] 起動失敗が再起動ループにならず、journal に秘密値がない。
- [ ] 停止後にリンクが外れ、SSH と対象外通信が維持される。
- [ ] 監視サービス停止時に保護がないことが利用者に分かる。

## 中止と対象外

VM 全体の cgroup へ attach しない。起動時の enforce 有効化は検証結果と具体的な運用範囲を確認してから別途判断する。

## 検証記録

未実施。実行した環境、入力、結果、未検証事項だけを追記する。

## 企業での限定試用計画との対応（2026-10-22）

M6は[W1](../enterprise/README.md)に対応し、保留・未着手を維持します。
前半は[W1-1/W1-2](../../docs/ENTERPRISE_TASKS.md)のJSON Lines契約と出力障害・欠落の検証、
後半はW1-3/W1-4の限定scope service・権限・ready・停止復帰です。
現行text/TUIと構造化ログを区別し、Snapshotの接続試行数を「遮断パケット数」と呼びません。
metrics HTTP、設定reload、配布署名、DaemonSet、bpffs保持はM6にまとめて追加せず、後続候補で別に判断します。
具体的な利用者・一サービス・一環境の採否（W0）と実装承認が前提です。

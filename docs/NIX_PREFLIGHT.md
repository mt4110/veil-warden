# Nixパッケージと読み取り専用の導入前診断

状態：CLI+BPFの一体パッケージと読み取り専用診断を実装し、ARM64 Linuxでビルド・非ロード検証済みです。
[検証記録・成果物ハッシュ・未確認事項](validation/nix-preflight-20261022/README.md)を参照してください。
実サーバーへの導入・サービス登録・BPFロードは行っていません。
現行loaderの専用VM制限は維持しています。このパッケージは一般サーバーへ監視を開始する機能ではありません。

## パッケージ構成

Linux用の名前付き出力 `veil-warden` に、同一のNix sourceから構築する次の成果物を含めます。
既存の既定出力（architecture-guard）とVM用ビルドスクリプトは変更しません。

| パス | 内容 |
| --- | --- |
| `bin/veil-warden` | CPUに対応するCLI。diagnoseはloaderより前に分岐 |
| `lib/veil-warden/veil-warden-ebpf` | pinned nightlyで構築するbpfel/v3オブジェクト |
| `share/veil-warden/manifest.json` | source store path、system、イベントABI、両ファイルのSHA-256、導入不可の状態 |
| `share/veil-warden/LICENSE.md` | プロジェクトのライセンス |
| `share/veil-warden/README.md` | この手順書 |

CLIとBPFの独立ビルド出力は `warden-cli` と `warden-ebpf` です。
Darwin版CLIは診断・模擬TUI用で、Linux監視用パッケージ `veil-warden` はDarwinに提供しません。
manifestのハッシュは組み合わせの確認用で、配布者の署名・SBOM・本番対応の認定ではありません。

既存Linux環境のリポジトリルートでの開発用ビルド手順:

```sh
nix --extra-experimental-features 'nix-command flakes' build .#veil-warden --option builders ''
nix --extra-experimental-features 'nix-command flakes' develop . --command python3 scripts/verify-package.py ./result
```

Git checkoutのflakeを使うため、追加したファイルはGitの追跡対象に含めてから実行します。
`path:.`はignoredな大きい成果物まで入力として取り込み得るため、上記では使いません。

これは開発用ビルドです。キャッシュからのみ取得する企業向け導入コマンド、署名検証、systemd登録は未実装です。
Nixのインストールや信頼設定の変更はこの手順に含めません。

BPFの標準ライブラリ依存は `config/bpf-stdlib-Cargo.lock` に固定しています。
これはrust-overlayで指定したnightly 2025-12-01のrust-src内 `library/Cargo.lock` の写しです。
ビルド時にtoolchain内の原本と照合し、不一致なら停止します。依存はNixで取得し、Cargoはoffline/lockedで実行します。
元のRustソースのライセンスは[公式Rustリポジトリ](https://github.com/rust-lang/rust)を参照してください。

## 診断を使う

既存のsystemdサービスのcgroupは、必要なら管理者が以下の読み取り操作で確認します。
診断自身はsystemctlや外部コマンドを起動しません。

```sh
systemctl show example.service --property=ControlGroup --value
```

得られた `/system.slice/example.service` のような、cgroup mountからのパスを明示します。
`/sys/fs/cgroup` は先頭に付けません。パスからサービス所有者やPIDの所属を認定する機能はまだありません。

```sh
./result/bin/veil-warden diagnose --cgroup /system.slice/example.service --lang ja
./result/bin/veil-warden diagnose --cgroup /system.slice/example.service --lang en
./result/bin/veil-warden diagnose --help
./result/bin/veil-warden diagnose -h
```

CLIのみをローカルビルドして試す場合も同じ構文です。

```sh
cargo run --locked -p veil-warden -- diagnose --cgroup /system.slice/example.service --lang ja
```

root権限は要求しません。ファイルへの書き込み、サービス操作、cgroupの作成・移動、mount、BPF syscall、ネットワーク接続は行いません。
診断本体が読むのはLinuxのkernel release、hostname、cgroup controllersと、指定パスのメタデータです。
各テキスト読取は64 KiBまで。argv・環境変数・秘密ファイルを走査せず、生のhostname等は出力しません。
CLIの起動自体に必要な共有ライブラリ等のOSによる読取は別です。

root cgroup、親slice、パス逸脱、symlinkの対象、既知のSSHサービス・自身を対象にしません。
任意の管理サービス名を網羅する保証ではありません。実際の管理経路の同定は後続の導入設計で行います。
symlink/不在/読取不能は未確認となり、診断中の変更もあり得るため将来の適用許可には使用できません。

## 結果の読み方

出力は固定キーと日英の説明を持つテキストです。先頭に以下を必ず出します。

```text
schema=veil-warden-preflight-v1
read_only=true
bpf_loaded=false
apply_supported=false
deployment_ready=false
```

| 判定 | 意味 |
| --- | --- |
| `candidate` | その項目の限定的な静的条件だけを確認。全体の適合判定ではない |
| `blocked` | 現行実装の制約に合わない |
| `unknown` | この読み取り診断では確認できない |

| 終了コード | 意味 |
| --- | --- |
| 0 | ヘルプの表示だけ。診断合格には使わない |
| 1 | 既知の不適合あり |
| 2 | 入力不正。診断を開始しない |
| 3 | 既知の不適合がなくても、実動作の証拠は未確認 |

一般Linuxサーバーでは `loader_scope=blocked`、Macでは `platform=blocked` が想定されます。
専用VMでもBPF未ロードのため、ロード権限・LSM・競合・両フック・停止復帰は確認できず、終了コード3です。
hostnameの変更で制限を回避しないでください。
この診断では「起動できる」「保護できる」「企業導入可能」とは判定しません。

## 次段階との境界

対象サービスの安定した同定と再検証、一般サーバー向けloader、JSONイベント、systemd readiness、停止・撤去は未実装です。
今回は設定生成や適用コマンドを出しません。実通信の試験やx86_64での動作実証には別途許可と証拠が必要です。

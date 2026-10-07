# veil-warden 実行ガイド

## 前提

Apple Silicon Mac と既存の Nix を使います。初回の取得にはインターネット、数 GiB 程度の空き容量が必要です。Nix が未導入なら、公式の導入方法を確認してから導入してください。このプロジェクトのスクリプトはホスト権限を拡大しません。

初回は README の「クイックスタート」を使います。`./scripts/install-cli.sh` 後、新しいターミナルから `veil-warden` でMacの模擬TUIを開けます。VMの実監視は `veil-warden tui` です。[利用場面と操作体験](USE_CASES.md)で両者の違いを確認してください。下記は個別スクリプトを使う開発・検証用の手順です。mise は使用しません。

```sh
nix develop "path:$PWD"
./scripts/check.sh
```

M0 の stable Rust と整形・リントは固定 nixpkgs 由来です。M1 の BPF ビルドは別の `#bpf` shell を使います。nightly-2025-12-01（LLVM 21）と bpf-linker 0.9.15 を組み合わせ、通常のCLI・ガードは stable 1.95.0 のままです。

## 構築用 VM

```sh
./scripts/bootstrap-builder.sh
```

このターミナルを開いたままにします。READMEの個別スクリプト手順の1つ目で起動します。公式のキャッシュ済み NixOS Linux builder を、ホスト設定を変更する installer を通さず起動します。launcher は固定した公式生成物の形を確認し、loopback・2 CPU・4 GiB・ディスク32 GiBへ変更します。形が変わった場合は起動前に拒否します。

SSH はホスト `127.0.0.1:32222`、既知の公開ホスト鍵を検証します。生成した秘密鍵は状態ディレクトリ内に保持し、VM には公開鍵だけを渡します。

## 専用 VM のビルドと起動

別ターミナルで README の個別スクリプト手順の2つ目を実行します。`start-vm.sh` が以下を続けて行います。

```sh
./scripts/build-vm.sh
./scripts/run-vm.sh
./scripts/ssh-vm.sh 'sudo warden-preflight'
```

最後のpreflightが通ると起動コマンドは戻り、専用VMはバックグラウンドで動き続けます。出力は `sandbox/quickstart-vm.log` にあります。ビルド用に flake・Cargo 設定、config、infra、tests、tools をローカル構築用 VM へコピーします。Linux の中で kernel/initrd/Nix store のイメージを作り、ホストへ戻します。Nix daemon の trusted-user 設定や remote builder 設定は不要です。

専用 VM は `127.0.0.1:32223` で SSH を受け、root は一時的なメモリ領域です。最初の公開ホスト鍵を取得した後はその起動の鍵を検証します。2 CPU・4 GiB、ホストと共有するのは公開鍵ディレクトリだけです。

## 機能と通信範囲の検証

起動後のpreflightは自動実行されます。手動で再確認する場合は次を使います。

```sh
mkdir -p artifacts/m0
./scripts/ssh-vm.sh 'sudo warden-preflight' > artifacts/m0/preflight.json
```

次を確認します。

- ARM64、cgroup v2、kernel BTF。
- cgroup_skb、cgroup_sock_addr、RingBuf、HashMap、per-CPU Array の利用可能性。
- 管理用 SSH がテスト slice の外にあること。
- systemd がテストクライアントを専用 slice に置いてからソケットを作ること。
- IPv4/IPv6 loopback の合成文字列がサーバーに届くこと。

`bpftool feature probe` は一時的な BPF program/map による capability 試験を行います。自作の通信プログラムを attach する段階ではありません。cgroup BPF link の実 attach は M1 で検証します。

## 停止と復帰

```sh
./scripts/ssh-vm.sh 'sudo poweroff'
./scripts/run-vm.sh
```

専用 VM は再起動で宣言した初期状態へ戻ります。同じ preflight を再実行してください。ディスクやログを自動削除しません。

構築用 VM はそのコンソールで `shutdown now` を実行します。公開鍵・ローカル状態は保持されます。構築用ディスクの破損や手動再作成が必要なら、既存ディスクを勝手に削除せず、対象と復旧方針を確認してください。

## 状態とトラブル対応

状態の既定先は `$HOME/.cache/veil-warden-m0`、`WARDEN_STATE_DIR` で別の専用ディレクトリを指定できます。秘密鍵は private、公開鍵は public-keys、構築用状態は bootstrap、専用 VM の各起動は sandbox/runs に置きます。既存の user data を置かないでください。

ポートが使用中なら既存の VM を確認し、重複起動しません。SSH が起動前なら、コンソールで起動を確認してから再試行します。鍵検証に失敗したら検証を無効にせず、対象の起動と known_hosts を確認します。

Markdown/Rust/Nix の検証失敗は原因を修正してから再実行します。M0 の環境構築が3時間の作業枠で成立しない場合は結果と阻害要因を記録して停止します。

## M1 のビルドと実行

Apple Siliconでは、構築用VMと専用VMを起動し、ホストのリポジトリで実行します。

```sh
./scripts/build-counter-vm.sh
./scripts/test-counter-vm.sh
```

ビルドスクリプトは明示したソースだけを構築用VMへコピーします。成果物は状態ディレクトリの `m1/build-日時-PID`、受け入れ試験のJSONは `artifacts/m1/acceptance-日時-PID.json` に保持します。テスト用コピーは専用VMの `/home/warden/warden-m1-日時-PID` に置きます。既存のホスト成果物を削除しません。

構築用VMのCargoキャッシュは `/home/builder/.cache/veil-warden-build/target` を共用します。日時ごとのソースディレクトリ内には `target` を作りません。依存のコンパイル結果を再利用し、Nixへのソース取り込みに中間成果物が混ざることを避けます。ビルド全体をロックし、完了した3成果物だけを各ソースの `output` にコピーしてからMacへ取得します。ソース・成果物・検証記録は保存するため、保存量が無制限に増えない保証ではありません。容量確認と整理は[構築VMの保守](BUILDER_MAINTENANCE.md)を参照してください。

受け入れ試験はIPv4/IPv6で8個ずつ合成UDPデータグラムを送り、受信・カウント増加を確認します。対象外の同じ通信ではカウンタが増えないこと、通常終了・SIGTERM・SIGKILLで自作リンクだけが消えること、管理SSHが維持されることも確認します。

CLIを手動で使う場合、転送先のパスを試験スクリプトから確認して、専用VM内で実行します。

```sh
sudo systemctl start warden-test.slice
sudo /home/warden/warden-m1-日時-PID/veil-warden \
  --object /home/warden/warden-m1-日時-PID/veil-warden-ebpf \
  --interval-ms 1000 --samples 10
```

`--samples 0`（既定）はSIGINT/SIGTERMまで継続します。対象cgroupは固定で、変更する引数はありません。クライアントは `systemd-run --slice=warden-test.slice` で所属してからソケットを作ります。CLI自体は対象sliceの外で実行します。ヘッダやペイロードの収集・書き換え、宛先別集計は行いません。

Linuxでソースからビルドする場合は `./scripts/build-counter.sh` を使います。完成したCLI・BPF・部分attach試験用オブジェクトは `artifacts/build/` にコピーします。`WARDEN_BUILD_OUTPUT` で出力先を変更できます。キャッシュは既定で `${XDG_CACHE_HOME:-$HOME/.cache}/veil-warden-build/target`、`CARGO_TARGET_DIR` で変更できますがソースディレクトリ内は拒否します。自分でビルドしたオブジェクトだけをロードしてください。任意のオブジェクトの安全性をCLIが証明する機能はありません。

起動時の読み込み・Map・attach失敗はエラー終了します。既存のsystemdファイアウォールと共存するcgroup BPF linkを使い、既存programを上書き・解除しません。`/run/veil-warden-counter.lock` のFDロックで同じCLIの二重起動を拒否します。ロックファイルが存在していても、終了後はFDロックが解放されて再起動できます。

## M2 接続監視の実行

M1と同じ構築用・専用VMを起動し、ホストのリポジトリで実行します。

```sh
./scripts/build-counter-vm.sh
./scripts/test-connect-vm.sh
```

共通ビルドは通常のBPF/CLIに加えて、connect6を欠く部分起動試験専用fixtureを構築します。fixtureを通常の監視用に使わないでください。受け入れ試験は専用VMの `warden-m2-日時-PID` に成果物をコピーし、合成loopback通信だけを使います。JSONは `artifacts/m2/acceptance-日時-PID.json` に保持します。

手動でCLIを使う場合、VM内の転送先パスを指定します。

```sh
sudo systemctl start warden-test.slice
sudo /home/warden/warden-m2-日時-PID/veil-warden connect \
  --object /home/warden/warden-m2-日時-PID/veil-warden-ebpf \
  --interval-ms 1000 --duration-ms 10000
```

`--duration-ms 0`（既定）はSIGINT/SIGTERMまで継続します。`--samples` はM1専用です。診断用 `--reader-delay-ms 0..1000` はRingBuf受信を遅らせ、欠落試験に使います。通常は0のまま使ってください。

出力の `attempt` は接続試行、`decision=allow` はこのフックの判断、`connection_result=unknown` は実際の接続結果を取得していないことを意味します。拒否された接続も記録されます。`comm_status=best_effort` は現在のprocから読んだ名前、`unavailable`/`invalid` は未取得状態です。名前が不明でもイベントを継続します。

`stats` はattempted、emitted、ring_dropped、decoded、decode_errors、queue_dropped、displayedを分けて表示します。監視モードではRingBuf予約失敗でも通信を許可し、表示キュー満杯でも受信を止めません。表示workerが終了した場合はエラー終了してリンクを解放します。最終statsは通常終了・SIGTERMで表示し、SIGKILLでは出ません。

片方のフックで失敗したらCLIはready表示をせず終了し、取得済みFD linkを解放します。終了時は両リンクを解除してから残りのイベントを処理します。stdoutが詰まると最終表示・終了待ちが遅れることがあります。SIGKILLでもkernelがFDを閉じるため、自作のアタッチは残りません。

## M3 接続拒否と解除の実行

M1/M2と同じ構築用・専用VMを起動し、ホストで実行します。

```sh
./scripts/build-counter-vm.sh
./scripts/test-policy-vm.sh
```

JSONは `artifacts/m3/acceptance-日時-PID.json` に保持します。合成loopbackだけを使い、IPv4/IPv6拒否、解除、対象外通信、容量超過、既存接続、部分起動と終了後の復帰を検査します。

手動操作は、転送されたバイナリとobjectの実パスに置き換え、専用VM内で行います。

```sh
sudo systemctl start warden-test.slice
sudo /home/warden/warden-m3-日時-PID/veil-warden connect \
  --object /home/warden/warden-m3-日時-PID/veil-warden-ebpf \
  --enforce --deny 127.0.0.1 8443
```

`attached ... links=2 ... policy_mode=enforce` が出るまで対象クライアントを起動しません。これは両フックの準備完了です。`--enforce` を省くと監視のみで、`--deny` や稼働中のルール追加は拒否されます。`--enforce` 単独ではルールがゼロで、すべて許可します。起動前に一つのルールを設定する `--deny IP PORT` は一回だけ指定できます。

別のVMシェルで同じバイナリを使います。

```sh
sudo /home/warden/warden-m3-日時-PID/veil-warden policy list
sudo /home/warden/warden-m3-日時-PID/veil-warden policy add ::1 8443
sudo /home/warden/warden-m3-日時-PID/veil-warden policy remove ::1 8443
```

ルールはIPリテラルと1..65535のportで指定し、TCPだけに適用します。DNS名、CIDR、port範囲は使えません。IPv4-mapped IPv6のルールはIPv4へ正規化します。最大16件で、重複追加、存在しないルールの解除、容量超過は非ゼロ終了と `error` 応答になります。応答の `mode`・`rules`・一覧が現在のMap状態です。成功応答の後に開始する新規接続で結果を確認し、更新と同時進行のconnectを厳密に順序付けたとは扱いません。

拒否イベントは `decision=deny policy_id=非ゼロ`、許可は `decision=allow policy_id=0` です。クライアントの拒否errnoは固定カーネルで `EPERM`（1）でした。`connection_result=unknown` は実接続結果を追跡していない意味のままです。`stats` に `denied` を追加し、ログ欠落と独立して拒否数を計数します。

通信復帰は `policy remove`、または監視プロセスの通常終了・SIGTERMで行えます。SIGKILLでもkernelがFDを解放し、自作linkと制御endpointは残りません。ルールは永続化せず、再起動時は再登録します。既存接続はルール追加後も継続するため、すべての通信を停止する用途には使えません。UDP/QUICや秘密のスキャンも対象外です。

制御socketはabstract Unix socketでファイルを作らず、root以外の要求に応答しません。コマンド側も接続相手のUID 0を確認し、非rootの偽サーバーを拒否します。要求は256 byte、通信は2秒で打ち切ります。制御要求の処理中はログ受信が遅れ得ますが、kernelの拒否判断は待ちません。接続先が応答しない場合やCLIが停止した場合は制御コマンドも非ゼロで終了します。応答喪失時に操作結果を推測せず、`policy list` で状態を再確認してください。

## M4 TUIの実行

Macで操作だけを試す場合は `veil-warden tui --dry-run` を使います。`n`で接続再試行を模擬します。実通信・実拒否は行いません。実端末による模擬TUIの回帰試験はApple Silicon Macの `scripts/check.sh` に含まれ、`python3 tests/host/test_dry_run.py ~/.local/bin/veil-warden` でも実行できます。以下はVM内の実監視です。

構築用・専用VMを起動し、最新の成果物をビルドした後、実端末からホストで実行します。

```sh
./scripts/build-counter-vm.sh
./scripts/tui-vm.sh
# 拒否・解除も試す場合
./scripts/tui-vm.sh --enforce
```

起動helperは自作CLI/objectだけを専用VMへ転送し、SSHのPTYを割り当てます。既定はobserveです。CLIをVM内で直接使う場合は `connect --object PATH --tui` を指定します。stdin/stdoutがTTYでなければ、BPFをattachする前にエラー終了します。

画面上部は現在モード、準備状態、固定scopeです。幅100列以上は左右2ペイン、それより狭ければ上下に配置します。60列×20行以上を目安にし、狭すぎる場合はscopeと拡大案内を表示します。接続履歴の許可/拒否は色に加え、文字で表示します。接続成功や通信の安全性の判定ではありません。IPv4-mapped IPv6はルールと同じIPv4宛先表記に正規化して表示します。

| キー | 操作 |
| --- | --- |
| Tab | 接続履歴 / ルールの選択ペインを切替 |
| ↑↓ または j/k | 選択ペインの行を選択 |
| b | 選択した接続履歴の宛先tupleを拒否する操作を準備 |
| d | 選択したルールの解除を準備 |
| Enter | 画面の確認欄に表示した宛先と操作を実行 |
| Esc | 確認中の操作を取消 |
| q / Ctrl+C | 監視を終了し、接続制御と端末設定を復帰 |

b/dだけではMapを変更しません。確認欄の宛先を確かめてEnterを押します。確認中に新しいイベントが届いても、対象tupleを保持します。observeでは更新しません。成功・容量超過・重複・対象なしなどの状態を文字で表示し、失敗後も実Mapの一覧を保持します。TUIからの変更は所有するcontrollerを直接呼び、別デーモンとのIPCは使いません。

履歴は128件、受信queueと名前解決後の表示queueは各128件、操作queueは8件です。ring/decode/queue/uiの欠落を分けて表示し、履歴入替は画面保持範囲から外れた件数です。commの解決は専用workerで行い、描画やRingBuf受信と分けます。未取得名を安全と判定しません。

通常終了、Ctrl+C、SIGTERM、期限付き終了ではBPFリンクを解除してから端末を復元します。起動・描画失敗でも復元を試み、原因をエラーとして返します。SIGKILL・プロセスabort・端末そのものの消滅では復元処理を保証できません。SIGKILL後もBPFリンクはkernelのFD解放で解除されます。必要なら、その端末で `stty sane` と以下を実行して表示を戻してください。

```sh
printf '\033[?1049l\033[?25h'
```

実PTYの受け入れ試験は `./scripts/test-tui-vm.sh`、短いCLIデモは `./scripts/demo-policy-vm.sh` です。描画失敗fixtureはテストバイナリだけに含め、通常のCLIからは起動できません。

## M5の実行

M5は`--scan-argv PID:START_TICKS`で指定した専用cgroupの合成プロセス1件を、attach前に一度だけ評価します。既定ではargvを読みません。値を含むFindingを生成せず、rule ID・件数・評価状態だけを返します。検知からルールを追加せず、未評価と検知なしを区別します。資源上限、PID/FDの確認、既知の見逃しと復帰手順は[仕様と限界](SECRET_WARNING.md)を参照してください。

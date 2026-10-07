# 構築VMのディスクとビルドキャッシュ

[実行ガイド](GUIDE.md) / [文書一覧](README.md)

## VMの方式

Mac上でQEMUのARM64 Linux VMを2台使います。構築用は公式NixOS Linux builderを基にした永続ディスク、動作確認用は専用NixOS設定です。起動設定のアクセラレータは `hvf:tcg`（AppleのHypervisor.frameworkを優先し、利用できない場合はTCG）です。Virtualization.frameworkで直接VMを作る構成や、Docker/Colima経由の構成ではありません。[QEMU公式のアクセラレータ説明](https://www.qemu.org/docs/master/system/introduction.html)

構築用の設定は2 CPU・4 GiBメモリ・32 GiBディスクです。Macの `bootstrap/root-32g.qcow2` がLinux側の `/dev/vda` になります。動作確認用のread-only `store.img` とは別です。VMを停止・起動しても構築用のビルドキャッシュは残ります。

## 今後の保存方法

| 保存対象 | 場所と方針 |
| --- | --- |
| 構築VMのソース | `/home/builder/warden-m1-日時-PID` に世代別で保存 |
| Cargoの中間成果物 | `/home/builder/.cache/veil-warden-build/target/generations/<内容ハッシュ>`。同じ入力だけ再利用 |
| 完成したLinux CLIとBPF 2個 | 各ソースの `output` にコピーし、Macへ取得 |
| Macの成果物とソースアーカイブ | `~/.cache/veil-warden-m0/m1/build-日時-PID` に保存 |
| 検証記録 | checkoutの `artifacts/`。保持する |
| Nixの依存・ソース | Nixが管理。手作業でstoreを削除しない |

`CARGO_TARGET_DIR` はCargoの既存機能です。[Cargo公式のキャッシュ説明](https://doc.rust-lang.org/cargo/reference/build-cache.html)を参照してください。`CARGO_TARGET_DIR` はこのスクリプトではキャッシュの基点として扱い、その下の `generations/<内容ハッシュ>` をCargoへ渡します。Git履歴や更新時刻ではなく、ローカルソース・manifest・lock・toolchain・設定・ビルドスクリプト等の内容からキーを作ります。古い更新時刻のソースを復元しても、異なる内容のコンパイル結果は共用しません。Nixの `path:` ソース取り込み範囲の外へ置きます。同じソースディレクトリ内に大きな `target` を作ると、flakeの出力用filterより前のソース取り込みにも混ざり得るためです。ビルド全体と完成成果物のコピーは一つのロックで保護します。

ネイティブCLIはNixのRustコンパイラのhost tripleを明示し、ターゲット別の出力をコピーします。ビルド中に入力が変わった場合は完成成果物のコピーを拒否します。使用したキャッシュの実パスとキーは `output/cargo-target-dir`、`output/build-source-key`（直接ビルド時は指定した出力先）に保存します。TUI試験のビルドもこの実パスを使います。

Cargoのダウンロードキャッシュは共有しますが、コンパイル済み依存も内容ハッシュごとに保持するため、新しい世代では再コンパイルと追加のディスク容量が必要です。既存の共通キャッシュは削除・移動しません。完成成果物も世代別に保持するので、容量が永久に増えない方式ではありません。OS・コンパイラ・依存の版が増える場合も容量確認が必要です。自動削除や自動GCは行いません。

## 容量の確認と整理

SSHポート32222が構築用、32223が動作確認用です。`scripts/ssh-vm.sh` は32223に接続するため、構築用の容量を見る場合は `scripts/build-counter-vm.sh` と同じ鍵・known_hosts・接続先を使います。秘密鍵の内容は表示しません。

まず `df -h /` と `du -x -h --max-depth=1 /home/builder /nix` で、作業ディレクトリとNix領域を分けて調べます。ソース・成果物・検証記録を残す必要と、再生成できるキャッシュを区別してください。

削除前に対象パス、使用量、目的、残すもの、復旧方法を示し、承認を得ます。過去の `target` は再ビルドできますが、即座に元のファイルへ戻るバックアップとは異なります。Nix storeの手作業削除、VMディスクの作り直し、鍵の削除を容量不足の応急処置にしません。

2026-10-07の整理では、32 GiBのディスクが満杯で、構築ユーザーのデータが約11 GiB、Nix領域が約21 GiBでした。承認された14個の古い `target`（du計測合計約8.05 GiB）を削除し、空きは6.5 GiBになりました。最新の約2.1 GiBの `target` は共通キャッシュへ移動して保持し、旧パスからシンボリックリンクで参照できるようにしました。計測の丸め、予約領域、共有・overlayの扱いがあるため、du合計とdfの空きは一致しません。ソース、Nix store、Cargoダウンロード、秘密鍵、Macの成果物、動作確認VMは削除対象に含めていません。

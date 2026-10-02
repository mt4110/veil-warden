# MIT License

Copyright (c) 2026 veil-warden contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## 日本語での補足

本リポジトリで新規作成するソース・文書は MIT ライセンスで提供します。[OSI が掲載する MIT 本文](https://opensource.org/license/mit)に対応した上記の英語本文がライセンス条件です。この補足は条件を変更しません。

第三者の依存、カーネル、VM 基盤、テンプレートにはそれぞれのライセンスが適用されます。MIT の指定で第三者の条件を置き換えません。現在の Rust ガード依存は syn と toml およびその推移依存で、今回の採用版について、Cargo メタデータ上の MIT / Apache-2.0 等の宣言を確認しました。unicode-ident は Unicode-3.0 の条件も持つため、その帰属も依存側のファイルで保持します。これは法務審査や脆弱性監査の完了を意味しません。

M1 で Aya テンプレートや eBPF コードを取り込む場合は既存の帰属を保持し、BPF プログラムのライセンス宣言・使用 helper の要件を個別に確認します。現時点では eBPF コードを配布していません。

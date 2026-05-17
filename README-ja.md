# MDLuma

MDLuma は Rust で書かれた、Windows 向けの軽量デスクトップ Markdown ビューアです。

![Windows 11](https://img.shields.io/badge/Windows%2011-0078D4?logo=windows11&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

**高速な起動と低メモリ使用量**に重点を置いており、Rust によるコンパクトな実装と [Sciter](https://sciter.com)（軽量 HTML/CSS/JS エンジン）によるレンダリングにより、WebView や Electron ベースのアプローチのオーバーヘッドを回避しています。

![screenshot](images/mdluma-top.jpg)

## 機能

- CommonMark（Markdown）構文のレンダリング（テーブル、ハイパーリンク対応）
- GFM 拡張（タスクリスト、シンタックスハイライトなど）
- ダーク/ライトテーマの切り替え
- 外部エディタとの連携
- フォントのカスタマイズなどの表示設定

## ビルド

現在 `x86_64-pc-windows-msvc` をターゲットとしています。

必要環境:

- Rust ツールチェーン（edition 2021、rust-version 1.80+）
- Windows ビルドツール
- Sciter.js SDK（[開発者リソース](#開発者リソース)を参照）

ビルド:

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

テスト:

```bash
cargo test
```

## 開発者リソース

このプロジェクトはデスクトップ UI に [Sciter.js SDK](https://gitlab.com/sciter-engine/sciter-js-sdk) を使用しています。

開発環境のセットアップ:

1. [公式ダウンロードページ](https://sciter.com/download/)から Sciter.js SDK をダウンロード
2. 内容を `vendor/sciter-js-sdk-main/` に展開し、ランタイム DLL が `vendor/sciter-js-sdk-main/bin/windows/x64/sciter.dll` に配置されるようにする

Sciter SDK のドキュメント（`docs/` フォルダ）は `vendor/sciter-js-sdk-main/docs/` に配置することで、Sciter の API やビヘイビアなどのオフラインリファレンスとして利用できます。

> **注:** このリポジトリでは `sciter.dll` のみを追跡しています。完全な SDK は Sciter のライセンス条件に従い別途入手する必要があります。

### sciter.dll の更新

`sciter.dll` を新しいバージョンに更新する際は、bindgen を使用して Rust バインディングを再生成する必要があります。手順については `tools/bindgen/README.md` を参照してください。

## ランタイムに関する注意事項

MDLuma は組み込みデスクトップ UI に Sciter ランタイムを使用しています。

実行時、アプリケーションに必要な Sciter ファイルと UI アセットがパッケージ化されたアプリケーションと共に配置されている必要があります。

### Sciter DLL のバージョンチェック

MDLuma は起動時に Sciter DLL のバージョンチェックを行い、ビルド時に使用された API との互換性を確認します。

- DLL のフルバージョン（例: `6.0.3.18`）を取得し、デバッグ目的でログに記録します
- 互換性はメジャーバージョンとマイナーバージョンのみ（例: `6.0`）で検証されます
- メジャーまたはマイナーバージョンが期待されるバージョンと一致しない場合、MDLuma はユーザーフレンドリーなエラーメッセージを表示して即座に終了します

このアプローチにより、パッチ番号やビルド番号の違いは許容しつつ、API レベルの互換性を確保しています。

## 設定

設定は `%LOCALAPPDATA%\MDLuma\settings.json`（通常 `C:\Users\<user>\AppData\Local\MDLuma\settings.json`）に保存されます。

ほとんどの設定はアプリケーションの UI から変更可能であり、ファイルを直接編集する必要はありません。主な例外は以下の通りです:

- `content_max_width_px`: コンテンツの最大幅をピクセル単位で指定（デフォルト: `1040`、有効範囲: `640`〜`2400`）
- `cjk_friendly_emphasis`: `注意：__注意事項__` のようなケースで Comrak の CJK 対応強調構文解析をデフォルトで有効にします。より厳密な CommonMark/GFM 互換のアンダースコア解析を行いたい場合は `false` に設定してください

これらの調整が必要な場合は、`settings.json` に次のように追加または変更してください:

```json
{
  "content_max_width_px": 1100,
  "cjk_friendly_emphasis": false
}
```

## 技術スタック

- Rust
- [Comrak](https://github.com/kivikakk/comrak) — Markdown から HTML への変換
- [Sciter.js SDK](https://gitlab.com/sciter-engine/sciter-js-sdk) — HTML/CSS/JS ベースのデスクトップ UI レンダリング

## ライセンス

MDLuma プロジェクトのソースコードは以下のデュアルライセンスの下で提供されています:

- MIT
- Apache License 2.0

いずれかのライセンスを選択して本プロジェクトを利用できます。詳細は `LICENSE-MIT` および `LICENSE-APACHE` を参照してください。

## サードパーティランタイム: Sciter

MDLuma はデスクトップ UI のレンダリングに Sciter.js SDK を内部的に使用しています。

Sciter はサードパーティのコンポーネントであり、MDLuma プロジェクトのライセンスの対象外です。Sciter のランタイムファイル、SDK ファイル、または関連バイナリは、Sciter 独自のライセンスと再配布条件に従います。

MDLuma を Sciter ランタイムファイルと共に配布する場合は、適用される Sciter ライセンス条件への準拠の責任は使用者にあります。

参考として、Sciter SDK の配布物には Sciter の各部分に対する個別のライセンス文書が含まれています:

- `LICENSE` — Sciter SDK ソース配布ライセンス（BSD 3-Clause）
- `SCITER-ENGINE-EULA.md` — `sciter.dll` などのエンジンバイナリに対する Sciter Engine ランタイムライセンス条項

SDK パッケージおよび付属のライセンスファイルについては、公式 Sciter SDK ダウンロードページを参照してください:

- [https://sciter.com/download/](https://sciter.com/download/)

再配布、パッケージング、または商用/非商用の利用条件を評価する際は、特にアプリケーションに同梱されるランタイムバイナリに関する Sciter Engine EULA を注意深く確認してください。

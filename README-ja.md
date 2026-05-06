# MDLuma

MDLuma は Rust で書かれた Windows 向け軽量デスクトップ Markdown ビューアです。

![Windows 11](https://img.shields.io/badge/Windows%2011-0078D4?logo=windows11&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)
![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)

**高速起動・省メモリ** を特長としています。Rust でコンパクトに実装し、レンダリングには [Sciter](https://sciter.com)（軽量な HTML/CSS/JS エンジン）を利用することで、フル WebView ベースや Electron ベースのアプローチに比べてオーバーヘッドを抑えています。

## 機能

- テーブル、ハイパーリンクを含む基本的な Markdown 構文に対応
- ダーク / ライトテーマの切り替え
- 外部エディタ連携
- フォント変更などの表示設定

## 今後の予定

- Markdown 対応の拡充（GFM 拡張、タスクリスト、脚注など）

## ビルド方法

MDLuma は現在 `x86_64-pc-windows-msvc` をターゲットとしています。

要件:

- Rust ツールチェーン（edition 2021、rust-version 1.80 以上）
- Windows ビルドツール
- Sciter.js SDK（[開発者向けリソース](#開発者向けリソース) を参照）

ビルド:

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

テスト:

```bash
cargo test
```

## 開発者向けリソース

このプロジェクトはデスクトップ UI に [Sciter.js SDK](https://gitlab.com/sciter-engine/sciter-js-sdk) を使用しています。

開発環境のセットアップ手順:

1. [公式ダウンロードページ](https://sciter.com/download/) から Sciter.js SDK をダウンロードします
2. 内容を `vendor/sciter-js-sdk-main/` に展開し、ランタイム DLL が `vendor/sciter-js-sdk-main/bin/windows/x64/sciter.dll` に配置されるようにします

Sciter SDK のドキュメント（`docs/` フォルダ）も `vendor/sciter-js-sdk-main/docs/` に配置することで、Sciter の API や動作、詳細をオフラインで参照できるようになります。

> **注:** このリポジトリでは `sciter.dll` のみを追跡しています。SDK 全体は Sciter のライセンス条件に従って別途入手してください。

## ランタイムに関する注意

MDLuma は埋め込みデスクトップ UI に Sciter ランタイムを使用しています。

実行時には、必要な Sciter ファイルと UI アセットがアプリケーションと一緒に配置されている必要があります。

### Sciter DLL のバージョンチェック

MDLuma は起動時に Sciter DLL のバージョンチェックを行い、ビルド時に使用した API との互換性を確認します。

- DLL の完全なバージョン（例: `6.0.3.18`）を取得し、デバッグ目的でログに記録します
- 互換性はメジャーバージョンとマイナーバージョンのみで検証します（例: `6.0`）
- メジャーまたはマイナーバージョンが期待値と一致しない場合、ユーザーにわかりやすいエラーメッセージを表示して終了します

これにより、パッチやビルド番号の差異を許容しつつ、API レベルの互換性を確保しています。

## 技術スタック

- Rust
- [Comrak](https://github.com/kivikakk/comrak) — Markdown から HTML への変換
- [Sciter.js SDK](https://gitlab.com/sciter-engine/sciter-js-sdk) — HTML/CSS/JS ベースのデスクトップ UI レンダリング

## ライセンス

MDLuma プロジェクトのソースコードは、デュアルライセンスです:

- MIT
- Apache License 2.0

どちらかのライセンスを選択して利用できます。詳細は `LICENSE-MIT` および `LICENSE-APACHE` を参照してください。

## サードパーティランタイム: Sciter

MDLuma は内部で Sciter.js SDK を使用してデスクトップ UI をレンダリングしています。

Sciter はサードパーティコンポーネントであり、MDLuma プロジェクトのライセンスの対象外です。Sciter のランタイムファイル、SDK ファイル、関連バイナリには Sciter 独自のライセンスおよび再配布条件が適用されます。

MDLuma を Sciter ランタイムファイルとともに配布する場合、該当する Sciter のライセンス条件を遵守する責任は配布者にあります。

Sciter SDK 配布物には、Sciter の各部分に応じた以下のライセンス文書が含まれています:

- `LICENSE` — Sciter SDK ソース配布ライセンス（BSD 3-Clause）
- `SCITER-ENGINE-EULA.md` — `sciter.dll` などのエンジンバイナリに関するランタイムライセンス条件

公式の Sciter SDK ダウンロードページと、それに付属するライセンスファイルを参照してください:

- [https://sciter.com/download/](https://sciter.com/download/)

再配布、パッケージ化、商用・非商用の使用条件を評価する際は、特にアプリケーションに同梱するランタイムバイナリについて、Sciter Engine EULA を注意深く確認してください。

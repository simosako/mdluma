# Technology Stack

updated_at: 2026-05-05

## アーキテクチャ

MDLuma は、`Markdown -> HTML -> 埋め込み UI で描画` の流れを採るデスクトップビューアーです。Markdown の変換責務、HTML シェル生成、UI 表示、プラットフォーム固有処理を分離し、小さな境界でつなぎます。

Rust 側はアプリケーション制御とエラー処理を担い、UI は Sciter.js SDK を用いた HTML/CSS/JavaScript で構成します。外部ランタイムに依存する箇所は狭い FFI 境界に閉じ込めます。

## コア技術

- **言語**: Rust 2021
- **標準ターゲット**: `x86_64-pc-windows-msvc`
- **Markdown 変換**: Comrak
- **HTML レンダリング**: Sciter.js SDK
- **Windows リソース埋め込み**: `winresource`（Windows ビルド時）

## 技術方針

### Markdown

- Markdown は Comrak で HTML フラグメントへ変換する。
- GFM のうちビューアーに必要な拡張を有効にする。
- 出力は完全な HTML ドキュメントではなく、UI シェルへ差し込む本文断片として扱う。

### UI とランタイム

- UI は Sciter.js SDK を前提とし、古い Sciter.TIS 系のラッパーへ寄せない。
- Rust から Sciter へは小さなローカル FFI 層で接続する。
- UI 資産はローカル配布物のみを参照し、ネットワーク依存を持ち込まない。

### Sciter 実装方針

- Sciter 上の実装判断では、WebView や一般的なブラウザの挙動を前提にせず、まず Sciter の公式 docs / samples / headers を確認する。
- HTML, CSS, JavaScript を書くときも同じで、実装前に Sciter の docs / samples / headers を確認してから記述する。
- 特に `role`、intrinsic behavior、window 属性、drag-and-drop、selection、hyperlink のような機能は、DOM や CSS の見た目だけで判断せず、Sciter 固有のイベント面と責務分離を先に確認する。
- 何らかの処理を JS 側で自前実装する前に、その機能が Sciter 標準の behavior event や role で処理できないかを検討する。
- Rust で安定して扱える処理は Rust 側に寄せる。特にプラットフォーム連携、Sciter 標準 behavior の通知処理、セキュリティ境界の判定、アプリケーション状態管理は Rust 側を優先する。
- JS は禁止しないが、責務は薄く保つ。UI の見た目、軽量な入力補助、Rust 側 command の起点、Sciter 標準機能では表現しにくい局所的な振る舞いに集中させる。
- `xcall()` は UI から Rust へのイベント通知チャネルとして優先的に使う。クリックやトグルなどの UI 操作は、まず `xcall()` で Rust へ通知する経路を基本形とする。
- `xcall()` を受けた後の状態遷移と反映は Rust 側で実行する。特に theme や font など viewer 全体へ影響する設定反映は、JS 側の自律処理へ寄せず Rust 主導で扱う。
- 文書のスクロール位置や検索状態の保持が重要な操作では、HTML の全再読み込みより増分更新を優先する。必要な DOM 変更だけを適用し、表示状態を不必要にリセットしない。
- 既存の `xcall()` や custom event が別機能で成功していても、新しい UI 要素へ機械的に横展開しない。対象要素の Sciter behavior が異なるなら、まずその要素に対応する標準イベント経路を調べる。
- デバッグ時は「HTML が正しいか」「Sciter がどのイベントを出しているか」「Rust 側の event bridge まで届いているか」を層ごとに切り分ける。ログを増やす前に、そもそものイベント面の選択が Sciter に合っているかを見直す。

### エラー処理

- 起動前に DLL や必須アセットの存在を検証する。
- エラーはユーザー向け文言と診断向け情報を分離して扱う。
- 外部依存の失敗は早期に止め、原因を追える形で返す。

### デバッグログ

- デバッグログは debug ビルド時のみ有効にし、release ビルドの挙動や依存へ影響させない。
- 新しいデバッグ用出力は `eprintln!` へ直接書かず、`debug_log!` マクロを使って `%LOCALAPPDATA%\MDLuma\logs\` 配下へ記録する。
- `stderr` は起動失敗などの最終フォールバック用途に限定し、通常の診断ログ出力先として増やさない。

### テスト

- 単体テストを各モジュールに近接配置する。
- 外部ランタイムや UI 依存はフェイク実装や記録用実装で置き換えて振る舞いを検証する。
- テストでは起動フロー、エラー経路、GFM 変換、UI ブリッジの契約を重視する。

## 開発環境

### 必要ツール

- Rust toolchain
- Windows 向けビルド環境（Visual Studio Build Tools を含む）
- Sciter ランタイム配布物と `assets/`

`nmake.exe` や `link.exe` が見つからない場合は、PowerShell の `Enter-Vs` を使って環境を整える前提です。

`rg` コマンドが導入済であるので、`grep` を使うような場合は代わりに `rg` を使う。

### Sciter 関連情報

Sciter のドキュメントは `vendor/sciter-js-sdk-main/docs/md/`、サンプルは `vendor/sciter-js-sdk-main/`、ヘッダは `vendor/sciter-js-sdk-main/include/` を参照する。

### よく使うコマンド

```bash
rtk cargo build
rtk cargo test
rtk cargo build --release --target x86_64-pc-windows-msvc
```

## 重要な設計判断

- 軽量性と制御性を優先し、Sciter は既製の大きな Rust ラッパーではなく局所 FFI で扱う。
- 配布は単一 exe 固執ではなく、必要な Sciter DLL とアセットを同梱する。
- ローカルファイル閲覧に集中するため、UI シェルと本文 HTML の双方でローカル限定ポリシーを守る。

---
このファイルは依存一覧ではなく、技術選定と実装判断の基準を残すための永続メモリです。

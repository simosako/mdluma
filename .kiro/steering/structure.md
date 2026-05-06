# Project Structure

updated_at: 2026-04-29

## 組織方針

MDLuma は機能別ではなく、責務別に小さく分ける構成を基本とします。ビューアーの起動、ドキュメント読み込み、Markdown 変換、HTML シェル生成、UI 表示、プラットフォーム固有処理をそれぞれ分離し、依存方向を明確に保ちます。

新しいコードは、既存の責務境界へ自然に収まるなら新しい層や抽象を増やさない方針です。

## ディレクトリパターン

### アプリケーション中核
**場所**: `src/`  
**役割**: ビューアーの主要ロジックを責務ごとのモジュールに分けて保持する。  
**例**: `app.rs` は状態遷移と操作制御、`document.rs` は入力読み込み、`markdown.rs` は変換責務を持つ。

### UI シェル
**場所**: `src/ui/` と `src/html_shell.rs`  
**役割**: HTML/CSS/JS の UI 資産と、それを状態へ束ねるシェル生成を保持する。  
**例**: `src/ui/index.html` をテンプレートとして読み、`DefaultHtmlShell` が現在状態を埋め込む。

### ランタイム境界
**場所**: `src/sciter/`  
**役割**: Sciter ランタイム読み込み、FFI、ウィンドウ連携など外部 UI ランタイムとの境界を閉じ込める。  
**例**: `runtime.rs` はランタイム検証、`window.rs` は UI 連携、`ffi.rs` は低レベル API を担当する。

### プラットフォーム固有処理
**場所**: `src/platform/`  
**役割**: Windows 固有のファイルダイアログやウィンドウ装飾制御を隔離する。  
**例**: `windows_file_dialog.rs` と `windows_window_chrome.rs`。

### デザインと配布資産
**場所**: `design/` と `assets/`  
**役割**: 実装判断の元になる設計資料と、実行時に使うアイコン資産をコードから分離して保持する。  
**例**: `design/initialdesign.md`、`assets/light/*.svg`、`assets/dark/*.svg`。

## 命名規約

- **Rust ファイル**: 責務を表す `snake_case` または単一責務の短い名前を使う。
- **型・列挙体・トレイト**: `PascalCase` を使う。
- **実装型**: 抽象の役割が分かる具体名を付ける。例: `FileDocumentLoader`、`ComrakMarkdownRenderer`。
- **定数**: `SCREAMING_SNAKE_CASE` を使う。例: `APP_NAME`。

## インポートと公開

```rust
mod app;
mod markdown;

pub use markdown::{ComrakMarkdownRenderer, MarkdownRenderer};
use crate::sciter::window::ViewerUi;
```

- ルートの `lib.rs` で内部モジュールを宣言し、外部から使う主要型だけを `pub use` で再公開する。
- crate 内参照は `crate::...` を基本とし、依存関係を明示する。
- サブディレクトリのモジュールは `mod.rs` を入口にしてまとまりを作る。

## コード構成の原則

- トレイトで境界を定義し、既定実装を小さな具体型として差し込む。
- 起動時の組み立ては `lib.rs` のような上位入口で行い、下位モジュールは自分の責務へ集中させる。
- テストは実装の近くに置き、外部依存を直接叩かず契約と振る舞いを検証する。
- Windows 優先だが、Windows 固有コードは局所化し、将来の移植を難しくしない。

---
このファイルはディレクトリ一覧ではなく、どこに何を置くべきかを判断するための永続メモリです。

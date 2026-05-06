# 調査記録

## Summary
- **Feature**: `document-path-normalization`
- **Discovery Scope**: Extension
- **Key Findings**:
  - GUI のファイルダイアログと起動引数はどちらも最終的に `DocumentLoader::load` を通るため、パス正規化の責務は `src/document.rs` に集約できる。
  - 現在の `SourceDocument.path` は入力値をそのまま保持し、`base_dir` もその入力値の `parent()` から導出しているため、CLI の相対パスと GUI の絶対パスで読み込み済み状態の意味がずれる。
  - Rust 標準ライブラリの `std::path::absolute` はファイルシステムへアクセスせず絶対化でき、Windows では `GetFullPathNameW` 相当で `.` / `..` と区切りを整理する。一方 `std::fs::canonicalize` はシンボリックリンク解決と Windows の extended-length path 化を行うため、本仕様の境界に対して強すぎる。

## Research Log

### 既存のドキュメント読み込み経路
- **Context**: どこでパス形式が分岐しているかを確認する必要があった。
- **Sources Consulted**: `src/app.rs`, `src/document.rs`, `src/lib.rs`, `src/platform/windows_file_dialog.rs`, `src/startup_args.rs`
- **Findings**:
  - GUI 経由では `WindowsFileDialog` が `PathBuf` を返し、`AppController::open_selected_path` から `DocumentLoader::load` が呼ばれる。
  - 起動引数経由でも `start_viewer_with` から `AppController::prepare_startup_path` が呼ばれ、同じく `DocumentLoader::load` が入口になる。
  - `FileDocumentLoader` は `SourceDocument.path` に入力 `path` をそのまま入れ、`base_dir` も同じ入力から算出している。
- **Implications**: 正規化は UI 層や起動計画層ではなく、`DocumentLoader` 内で一度だけ行うのが最小変更である。

### 正規化 API の選定
- **Context**: 要件は「正規化済み絶対パス」を求めるが、シンボリックリンク解決は対象外である。
- **Sources Consulted**:
  - Rust standard library: `std::path::absolute` https://doc.rust-lang.org/std/path/fn.absolute.html
  - Rust standard library: `std::fs::canonicalize` https://doc.rust-lang.org/std/fs/fn.canonicalize.html
- **Findings**:
  - `std::path::absolute` は相対パスを現在ディレクトリ基準で絶対化し、Windows では `GetFullPathNameW` 相当で区切りと `.` / `..` を整理する。
  - `std::path::absolute` は symlink を解決せず、ファイルシステム存在確認なしで成功しうる。
  - `std::fs::canonicalize` は symlink を解決し、Windows では extended-length path を返しうる。
- **Implications**: 本仕様では `absolute` を正規化の第一段階に採用し、その結果を読み込み済み状態の正本パスとして扱う。`canonicalize` は境界外である。

### 既存状態モデルへの影響
- **Context**: 要件 1.4 は表示用ファイル名とは別に正規化済み絶対パスを保持することを求める。
- **Sources Consulted**: `src/app.rs`, `src/markdown.rs`, `src/html_shell.rs`
- **Findings**:
  - `SourceDocument` には `path` があるが、`RenderedDocument` は `file_name`, `base_dir`, `html_body` しか持たない。
  - `ViewerState` が保持する読み込み済みドキュメントは `RenderedDocument` なので、現状ではロード後の正規化済み絶対パスを保持できない。
  - `html_shell` はファイル名表示のみを参照しており、保持パス追加によって表示文言を変える必要はない。
- **Implications**: `RenderedDocument` に正規化済み絶対パスを追加し、レンダラが `SourceDocument` から引き継ぐ必要がある。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| `AppController` で GUI/CLI ごとに正規化 | `open_selected_path` と `prepare_startup_path` の両方で前処理する | 実装位置が見えやすい | 経路重複が発生し、将来の読み込み経路追加で漏れやすい | 採用しない |
| `startup_args` / `WindowsFileDialog` 側で正規化 | 入力元ごとに正規化してから渡す | 呼び出し元に近い | GUI と CLI の責務が分散し、状態モデル要件 1.4 を満たす保証が弱い | 採用しない |
| `DocumentLoader` で正規化 | 読み込み入口で正規化済み `SourceDocument` を生成する | 単一責務、経路非依存、最小変更 | ローダーがパス確定失敗も扱う必要がある | 採用 |

## Design Decisions

### Decision: パス正規化責務を `FileDocumentLoader` に置く
- **Context**: GUI と起動引数の経路差を内部状態から消す必要がある。
- **Alternatives Considered**:
  1. `AppController` が各入口で正規化する
  2. `FileDocumentLoader` が読み込み直前に正規化する
- **Selected Approach**: `FileDocumentLoader::load` の先頭で入力パスを正規化済み絶対パスへ変換し、そのパスを `SourceDocument.path` と `base_dir` の唯一の情報源にする。
- **Rationale**: 両経路がすでに同じローダーへ収束しており、責務の重複がない。
- **Trade-offs**: ローダーの前処理責務は少し増えるが、新しい抽象や分岐を増やさずに済む。
- **Follow-up**: GUI と CLI の双方で `RenderedDocument.path` と `base_dir` が一致するテストを追加する。

### Decision: 読み込み済み状態に正規化済み絶対パスを保持する
- **Context**: 要件 1.4 は表示用ファイル名と別に保持パスを要求する。
- **Alternatives Considered**:
  1. `SourceDocument` のみ保持し、`RenderedDocument` では捨てる
  2. `RenderedDocument` に `path` を追加して `ViewerState` で保持する
- **Selected Approach**: `RenderedDocument` に `path: PathBuf` を追加し、`MarkdownRenderer` が `SourceDocument.path` をそのまま伝播する。
- **Rationale**: ビューアー状態がロード後も正本パスを失わず、今後の相対参照解決も `current_document()` から辿れる。
- **Trade-offs**: テスト fixture 更新箇所は増えるが、状態の意味が明確になる。
- **Follow-up**: `RenderedDocument` を直接生成する既存テストを更新する。

### Decision: 正規化失敗は既存の `FileRead` エラー系へ畳み込む
- **Context**: 本仕様は失敗時に読み込み済み状態へ進めないことを求めるが、新しい UI エラー分類は要求していない。
- **Alternatives Considered**:
  1. 新しい `ViewerError` variant を追加する
  2. 既存の `ViewerError::FileRead` に正規化失敗の診断を載せる
- **Selected Approach**: 正規化失敗は `ViewerError::file_read(path, "failed to normalize document path: ...")` で返す。
- **Rationale**: 既存のエラー表示と保持失敗の流れを再利用でき、UI 変更を避けられる。
- **Trade-offs**: ユーザー向け文言は「読めなかった」に統一されるが、診断ログで原因は区別できる。
- **Follow-up**: 正規化失敗時に `ViewerState::DocumentLoaded` へ進まないテストを追加する。

## Risks & Mitigations
- 相対パスの絶対化が現在ディレクトリ依存であることを実装者が見落とすリスク — `research.md` と `design.md` の両方で `std::path::absolute` 採用理由を明記する。
- `RenderedDocument` への `path` 追加で既存テスト fixture が広範に壊れるリスク — helper fixture を使って更新し、保持値の一致を合わせて検証する。
- 将来 `canonicalize` へ置き換えて symlink 解決や Windows extended-length path が混入するリスク — Boundary Commitments の対象外として明示する。

## References
- [Rust std::path::absolute](https://doc.rust-lang.org/std/path/fn.absolute.html) — symlink を解決せずに絶対化する標準 API。
- [Rust std::fs::canonicalize](https://doc.rust-lang.org/std/fs/fn.canonicalize.html) — symlink 解決と Windows extended-length path 化を伴うため今回の境界外。

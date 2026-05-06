# Research & Design Decisions

## Summary
- **Feature**: theme-toggle (settings persistence extension)
- **Discovery Scope**: Extension (existing theme-toggle with new persistence requirement)
- **Key Findings**:
  - アプリケーションに明示的なシャットダウンフックが存在しない — save-on-exit には追加インフラが必要
  - `debug_log.rs` で `%LOCALAPPDATA%\MDLuma\` ディレクトリパターンが確立済み
  - `serde` / `serde_json` は依存に含まれていない — 新規追加が必要
  - テーマ状態は `AppController.theme` フィールドで管理、`Theme::default()` = `Light`

## Research Log

### App Lifecycle & Shutdown
- **Context**: 設定の保存タイミング（終了時 vs 切替時）を決定するため、アプリケーションのライフサイクルを調査
- **Sources Consulted**: `src/lib.rs`, `src/app.rs`, `src/sciter/window.rs`
- **Findings**:
  - 起動: `run()` → `build_startup_controller()` → `AppController::run()` → `run_event_loop()`
  - 終了: ウィンドウクローズでイベントループ終了。`SciterWindow` の `Drop` でイベントハンドラ解放
  - 明示的なシャットダウンフックやクリーンアップコールバックなし
- **Implications**: save-on-exit を実現するには `Drop` impl またはウィンドウクローズイベントへのフックが必要。save-on-change なら追加インフラ不要で実装可能

### Existing Settings & File I/O Patterns
- **Context**: 設定ファイルの配置場所と I/O パターンを決定するため、既存のファイル操作パターンを調査
- **Sources Consulted**: `src/debug_log.rs`, `Cargo.toml`
- **Findings**:
  - ディレクトリ: `LOCALAPPDATA` 環境変数 → `MDLuma` サブディレクトリ（フォールバック: テンポラリディレクトリ）
  - ディレクトリ作成: `fs::create_dir_all` を使用
  - エラーハンドリング: `eprintln!` で stderr に出力、呼び出し元には `None` を返す
  - `serde` / `serde_json` は依存に含まれていない
- **Implications**: 設定ファイルも同じディレクトリパターンを使用。JSON 処理のため `serde` + `serde_json` を追加

### Dependency Analysis
- **Context**: JSON シリアライゼーション手法の選定
- **Sources Consulted**: `Cargo.toml`, crates.io
- **Findings**:
  - 既存依存: `comrak`（Markdown）、`windows-sys`（Win32）
  - `serde` + `serde_json` は Rust エコシステムの de facto 標準
  - 手動 JSON 構築は `{ "theme": "light" }` 程度なら可能だが、将来の拡張性に欠く
- **Implications**: `serde` + `serde_json` を採用。標準的で拡張性が高く、パースの堅牢性が保証される

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Save-on-change | テーマ切替時に即座に保存 | シャットダウンフック不要、クラッシュでも最新状態保持 | 将来の設定項目で保存トリガーが不明確な場合の対応が必要 | 選定 |
| Save-on-exit (Drop) | AppController::drop() で保存 | 終了時に常に最新状態を保存 | Drop 実行時のリソース状態に依存、順序保証なし | 不採用 |
| Save-on-exit (event) | ウィンドウクローズイベントで保存 | 明示的なタイミング制御 | Sciter イベントフローへの追加統合が必要 | 不採用 |
| 手動 JSON | フォーマット文字列で JSON 生成 | 依存追加なし | パース脆弱、拡張性なし | 不採用 |

## Design Decisions

### Decision: Save-on-Change による設定永続化
- **Context**: 設定保存のタイミングを決定
- **Alternatives Considered**:
  1. Save-on-exit via Drop — シャットダウンフック不要だが Drop 実行順序に依存
  2. Save-on-exit via window close event — 明示的だが Sciter 統合が必要
- **Selected Approach**: テーマ切替時に `SettingsFile::save()` を呼び出す
- **Rationale**: アプリにシャットダウンフックがないため save-on-exit は追加インフラが必要。save-on-change は既存フローへの最小限の追加で、クラッシュ時にも最新状態を保持
- **Trade-offs**: 将来の設定項目で保存トリガーが自明でない場合、追加の保存ポイントが必要
- **Follow-up**: 新しい設定項目追加時に保存タイミングを見直す

### Decision: serde + serde_json 採用
- **Context**: JSON シリアライゼーション手法の選定
- **Alternatives Considered**:
  1. 手動 JSON 構築 — 依存なし、拡張性なし
  2. `serde_json` のみ — derive マクロなしで手動実装
- **Selected Approach**: `serde`（derive feature）+ `serde_json` を dependencies に追加
- **Rationale**: Rust の標準的な JSON 処理。`#[serde(default)]` で前方互換性を容易に確保。将来の設定項目追加時に構造体にフィールドを追加するだけで対応可能
- **Trade-offs**: バイナリサイズ増加（~400KB）、コンパイル時間増加。既存の `comrak` に比べれば軽微
- **Follow-up**: リリースビルドでのバイナリサイズを確認

### Decision: ThemePreference 独立列挙体
- **Context**: 設定モジュールでテーマ値をどう表現するか
- **Alternatives Considered**:
  1. `ui::Theme` に serde derive を追加 — モジュール結合が増加
  2. 文字列で保存 — 型安全性なし
- **Selected Approach**: `settings` モジュールに `ThemePreference` 列挙体を定義し、`From` で `ui::Theme` と相互変換
- **Rationale**: 設定モジュールの独立性を保つ。serde 依存が UI モジュールに漏出しない
- **Trade-offs**: 変換コードが微量に増えるが、`From` impl で型安全に処理
- **Follow-up**: なし

### Decision: SettingsFile のエラー非伝播設計
- **Context**: 設定ファイル操作のエラーをどう扱うか
- **Alternatives Considered**:
  1. `Result` を返して呼び出し元で処理 — より柔軟だが呼び出し元のコード量が増加
  2. エラーを `SettingsFile` 内で消化 — 呼び出し元がシンプルになる
- **Selected Approach**: `load()` は常に `Settings` を返し、`save()` は `()` を返す。エラーは内部で `debug_log!` に出力
- **Rationale**: 設定ファイル操作は非致命的であり、要件（5.3: 終了を継続）に合致。AppController のコードがシンプルになる
- **Trade-offs**: エラーの呼び出し元でのハンドリングが不可能。将来的にリトライや代替保存先が必要な場合は設計変更が必要
- **Follow-up**: なし

## Risks & Mitigations
- serde_json によるバイナリサイズ増加 — Mitigation: comrak に比べ軽微。リリースビルドで確認
- 設定ファイルの書き込み中にクラッシュしてファイルが破損 — Mitigation: ファイルサイズが数十バイトのためリスク極低。将来の重要な設定追加時に atomic write を検討
- 複数 MDLuma インスタンスが同時に設定を書き込む — Mitigation: 通常使用では発生 unlikely。ファイルロックは現時点では不要

## References
- `src/debug_log.rs` — LOCALAPPDATA ディレクトリパターンの参照実装
- `src/app.rs` — AppController のテーマ状態管理
- serde documentation — https://serde.rs/ — `#[serde(default)]` および `#[serde(rename_all)]`

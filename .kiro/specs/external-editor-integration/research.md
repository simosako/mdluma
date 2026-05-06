# Research & Design Decisions

## Summary
- **Feature**: `external-editor-integration`
- **Discovery Scope**: Extension
- **Key Findings**:
  - `...` メニュー配下の項目は `src/sciter/window.rs` の `ViewerCommand::from_element_action()` により `data-action` から直接 Rust 側コマンドへ変換されているため、`External Editor` も同じ経路へ揃えるのが最小である。
  - 現在表示中ドキュメントのフルパスは `ViewerState::current_document()` から取得でき、`ErrorVisible` は直前の文書状態を保持するため、起動失敗時も閲覧継続の要件にそのまま適合する。
  - ウィンドウ終了は既存の `WindowChromeController::close()` を `SciterWindow` 内部で使えるが、`AppController` からは見えないため、UI 境界に `request_close()` 契約を追加するのが最小の責務分離になる。
  - Markdown 表示領域の右クリックメニューは Sciter の既定動作（Copy, Select All）であり、カスタムアイテム追加には `contextmenu` イベントのハンドリングと `<menu.context>` 要素の動的生成が必要。`name="edit:copy"` / `name="edit:selectall"` で既存動作を維持しつつ、独自の `<li>` で `External Editor` を追加できる。

## Research Log

### メニュー導線と無効化状態
- **Context**: `External Editor` を `Font` の下へ追加し、ファイル未オープン時は選択不能にする必要がある。
- **Sources Consulted**: `src/ui/index.html`, `src/sciter/window.rs`, `src/html_shell.rs`, `vendor/sciter-js-sdk-main/docs/md/DOM/Element/README.md`
- **Findings**:
  - 現在の `Font` 項目は JS の `xcall` を使わず、Sciter の element action から直接 `ViewerCommand` へ変換されている。
  - `HtmlShell` は `ViewerState` を参照してテンプレート置換できるため、メニュー項目の disabled 属性は描画時に決定できる。
  - Sciter の `Element.disabled` は要素と子孫を無効化し、`:disabled` 状態へ反映される。
- **Implications**:
  - `External Editor` も `data-action="external-editor"` と disabled 属性の組み合わせで表現し、`src/sciter/window.rs` に新しい command 変換を足す。
  - `app.js` に新しいイベント送出を増やさず、既存の menu popup パターンを維持する。

### 右クリックコンテキストメニューのカスタマイズ
- **Context**: Markdown 表示領域の右クリックメニューに `External Editor` を追加する必要がある。現在 Copy と Select All は Sciter の既定動作。
- **Sources Consulted**: `src/ui/app.js`, `src/ui/index.html`, `src/ui/styles.css`, `vendor/sciter-js-sdk-main/docs/md/DOM/out-of-canvas-elements.md`
- **Findings**:
  - 既存の右クリックメニューはカスタム実装ではなく Sciter のブラウザ既定動作。Copy は Ctrl+C ショートカット、Select All は `selectable` 属性による Sciter 標準挙動。
  - Sciter の `contextmenu` イベントハンドリングで `evt.source` に `<menu.context>` 要素を設定するとカスタムコンテキストメニューを表示できる。
  - `name="edit:copy"` / `name="edit:selectall"` を使うと既存の Copy/Select All 動作を維持しつつ、独自の `<li>` でカスタム項目を追加できる。
  - コンテキストメニューのクリックは `data-action` ではなく Sciter のセレクタベースイベント（`on click at menu.context>li.classname`）でハンドリングする。
  - `...` メニューと異なり、コンテキストメニューは動的生成のため、disabled 状態は右クリック時点で JS 側で判定する必要がある。
- **Implications**:
  - `src/ui/app.js` に `contextmenu` イベントハンドラを追加し、`<menu.context>` を動的生成する。
  - Copy/Select All は `name="edit:copy"` / `name="edit:selectall"` で既存動作を維持する。
  - `External Editor` の有効/無効は右クリック時に判定し、現在文書がない場合は `<li>` に `disabled` を付与する。
  - 文書ロード状態は `html_shell.rs` が markdown コンテナに設定する `data-document-loaded` 属性から JS 側で参照する。

### 設定永続化と既定値解決
- **Context**: 利用者設定ファイルで外部エディタを指定し、未指定時は Windows 既定で `notepad.exe` を使う必要がある。
- **Sources Consulted**: `src/settings.rs`, `.kiro/specs/theme-toggle/design.md`, `.kiro/specs/font-settings/design.md`
- **Findings**:
  - `Settings` は `theme` と `body_font` を持つ `#[serde(default)]` 構造体であり、controller は起動時に値を読み込んでセッション状態へ保持している。
  - 設定保存は best-effort で、失敗時もアプリケーションフローを止めない。
  - 既存の theme / body_font 更新処理は `Settings` 全体を書き戻すため、新しい設定項目も毎回保持して保存する必要がある。
- **Implications**:
  - `Settings` に `external_editor: Option<PathBuf>` を追加し、controller は起動時に読み込んだ値を保持する。
  - 未設定時だけ `notepad.exe` を使い、設定値が存在する場合は launch 失敗をそのまま可視化してフォールバックしない。

### 起動契約と終了制御
- **Context**: 外部エディタ起動成功時のみ MDLuma を終了し、失敗時はエラー表示して継続する必要がある。
- **Sources Consulted**: `src/app.rs`, `src/viewer_launcher.rs`, `src/errors.rs`, `src/platform/windows_window_chrome.rs`, `src/sciter/window.rs`
- **Findings**:
  - 外部プロセス起動は既に `std::process::Command::spawn()` パターンが存在する。
  - 現在の `ViewerUi` 契約にはウィンドウ終了要求がなく、close 操作は Sciter runtime 側に閉じ込められている。
  - `show_error_state()` は `current_document()` を保持したままエラー表示へ遷移できる。
- **Implications**:
  - 外部エディタ起動は専用の小さな launch 境界として切り出し、`AppController` では成功/失敗の分岐だけを扱う。
  - UI 終了は `ViewerUi::request_close()` を追加し、controller が Win32 を直接知らないまま success-only close を実現する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| `AppController` から直接 `Command::spawn()` | 起動処理を controller に直書きする | 実装ファイル数が最少 | launch 成功/失敗テストの差し替えが難しい。プロセス起動責務が controller に混ざる | 却下 |
| 既存 `viewer_launcher.rs` へ責務追加 | 子 viewer 起動と外部エディタ起動を同一 launcher 群へまとめる | 既存 launch 境界を再利用できる | `ViewerChildLauncher` の意味が崩れ、drag-and-drop 子起動と外部エディタ起動が同居して責務が曖昧になる | 却下 |
| `external_editor.rs` を新設し narrow な launch 境界を置く | 外部エディタ起動だけを専用モジュールで包み、controller は orchestration に集中する | 既存構造方針に沿う。テスト差し替えが容易。既存 child launch を汚さない | generic か wiring が 1 つ増える | 採用 |

## Design Decisions

### Decision: 外部エディタ起動を専用モジュールへ隔離する
- **Context**: 起動対象解決、`spawn()`、エラー変換を controller へ混ぜると、状態遷移と外部依存が同居してテストしづらくなる。
- **Alternatives Considered**:
  1. `AppController` に直接 `Command::spawn()` を記述する
  2. `viewer_launcher.rs` を汎用 launch モジュール化する
- **Selected Approach**: `src/external_editor.rs` に `ExternalEditorLauncher` と `ProcessExternalEditorLauncher` を置き、外部エディタ起動だけを担当させる。
- **Rationale**: 新しい責務境界を 1 つだけ増やし、既存の child viewer launch と混線させずに fake 実装テストを可能にする。
- **Trade-offs**: ファイルと依存が 1 つ増えるが、`AppController` の責務分離とテスト容易性が向上する。
- **Follow-up**: `app.rs` のテストで success/failure を記録する fake launcher を追加する。

### Decision: ウィンドウ終了は `ViewerUi` 契約として公開する
- **Context**: 起動成功時だけ viewer を終了したいが、`AppController` が Win32 window chrome 実装を直接参照すると platform 依存が漏れる。
- **Alternatives Considered**:
  1. `AppController` へ `WindowChromeController` を新しい依存として注入する
  2. `ViewerUi` に close 要求 API を追加する
- **Selected Approach**: `ViewerUi::request_close()` を追加し、`SciterWindow` が既存の `window_chrome.close()` を委譲実装する。
- **Rationale**: controller から見えるのは UI 境界だけになり、既存の runtime boundary 局所化方針を守れる。
- **Trade-offs**: `ViewerUi` を実装するテストダブルに close 記録が必要になる。
- **Follow-up**: close 失敗時は `ViewerError::Ui` として error state へ戻すテストを追加する。

### Decision: メニュー無効化は shell 描画時に決定する
- **Context**: ファイル未オープン時に `External Editor` を選択不能にする必要がある。
- **Alternatives Considered**:
  1. JS で表示後に動的 disable する
  2. `HtmlShell` が現在 state から disabled 属性を埋め込む
- **Selected Approach**: `HtmlShell` が `ViewerState::current_document()` に基づいて disabled 属性を埋め込み、controller 側でも no-document を防御する。
- **Rationale**: 起動直後の empty state と error state の両方で UI 表示が一貫し、追加の front-end 状態管理が不要になる。
- **Trade-offs**: 再描画タイミングでしか有効/無効は変わらないが、MDLuma は既に state change ごとに shell 再描画するため問題ない。
- **Follow-up**: `html_shell.rs` に empty state / loaded state の描画テストを追加する。

### Decision: 右クリックコンテキストメニューは動的生成し `data-document-loaded` 属性で有効/無効を判定する
- **Context**: Markdown 表示領域の右クリックメニューに `External Editor` を追加する。`...` メニューは shell 描画時に disabled を設定するが、コンテキストメニューは JS 側で動的生成される。
- **Alternatives Considered**:
  1. JS で Rust へ xcall して現在文書状態を問い合わせる
  2. `html_shell.rs` が markdown コンテナに `data-document-loaded` 属性を設定し、JS 側で判定する
- **Selected Approach**: `html_shell.rs` が markdown コンテナ要素に `data-document-loaded` 属性を設定し、`app.js` の `contextmenu` ハンドラがその属性から有効/無効を判定する。
- **Rationale**: 追加の xcall を発生させず、shell 描画時の状態属性をそのまま JS 側で参照できる。`...` メニューの disabled 判定と同じ状態源を使うため一貫性が保たれる。
- **Trade-offs**: 属性値が再描画まで更新されないが、MDLuma は state change ごとに shell を再描画するため問題ない。
- **Follow-up**: `html_shell.rs` に `data-document-loaded` 属性設定のテストを追加する。

## Risks & Mitigations
- 設定済み外部エディタパスが無効でも notepad へ自動フォールバックしてしまうリスク — 未設定時のみ `notepad.exe` を使い、設定値ありの失敗は `ExternalEditorLaunch` エラーとして固定する。
- 起動成功前に viewer を閉じてしまうリスク — `ExternalEditorLauncher::launch()` が `Ok(())` を返すまで `request_close()` を呼ばない。
- close 失敗で user が状態を見失うリスク — `current_document()` を保持した `ErrorVisible` へ遷移し、ユーザー向けエラーと診断ログを残す。
- menu item 無効化が UI だけに依存するリスク — shell で disabled を描画しつつ、controller 側でも no-document request を no-op 扱いして防御する。
- コンテキストメニューの `data-document-loaded` 属性が再描画前の古い状態を参照するリスク — MDLuma は state change ごとに shell を再描画するため実質的に同期している。controller 側の防御的 no-op が最終安全網。

## References
- `src/app.rs` — `ViewerState`, `AppController`, `show_error_state()`
- `src/sciter/window.rs` — `ViewerCommand` routing, `ViewerUi`, Sciter runtime bridge
- `src/settings.rs` — JSON settings persistence pattern
- `src/platform/windows_window_chrome.rs` — existing close contract used by runtime boundary
- `vendor/sciter-js-sdk-main/docs/md/DOM/Element/README.md` — `disabled` state semantics
- `vendor/sciter-js-sdk-main/docs/md/DOM/out-of-canvas-elements.md` — `menu.context` and `contextmenu` event pattern

---

# Gap Analysis (2026-05-05) — external-editor-integration

## 1) Current State Investigation

### 既存資産と責務境界
- `src/settings.rs`
  - `Settings.external_editor: Option<PathBuf>` が既に存在。
  - `SettingsFile::load()` / `save()` で `%LOCALAPPDATA%\MDLuma\settings.json` を読込/保存。
  - 保存失敗は `debug_log!` のみでユーザー通知なし（best-effort）。
- `src/app.rs`
  - 起動時に `settings.external_editor` を `AppController.external_editor` へ反映。
  - `open_in_external_editor()` は設定値があればそれを使用、なければ `notepad.exe` へフォールバック。
  - 起動成功時 `ui.request_close()`、失敗時 `show_error_state()` で継続。
- `src/external_editor.rs`
  - `ExternalEditorLauncher` trait + `ProcessExternalEditorLauncher` があり、`Command::new(executable).arg(document_path).spawn()` 実行。
- `src/ui/index.html` / `src/ui/app.js` / `src/sciter/window.rs`
  - `...` メニューに `External Editor` 項目は既存。
  - JS は `external-editor-requested` を `xcall`。
  - Rust 側で `ViewerCommand::ExternalEditorRequested` に変換済み。
- `src/platform/windows_file_dialog.rs`
  - `FileDialog` trait は `pick_markdown_file()` のみ。
  - Win32 `GetOpenFileNameW` 連携の実装は既にあり、owner HWND 連携・キャンセル/失敗分岐・テストも存在。

### 既存パターン
- 設定更新は `AppController` が `Settings` 全体を書き戻す方式（theme/font と同様）。
- UI コマンドは `data-action` / `xcall` / `ViewerCommand` の3層ルーティング。
- Platform 固有処理は `src/platform/` に隔離（構造方針と一致）。

## 2) Requirement-to-Asset Map（ギャップ付き）

| 要件 | 既存資産 | 判定 | 内容 |
|---|---|---|---|
| Req1.1 `External Editor` の下に `External Editor Setting` 表示 | `src/ui/index.html`, `src/html_shell.rs` | **Missing** | 新規メニュー項目と必要なら置換トークン追加が必要 |
| Req1.2 項目選択で OS 標準ファイル選択ダイアログ表示 | `src/ui/app.js`, `src/sciter/window.rs`, `src/platform/windows_file_dialog.rs` | **Missing** | 新規 action/xcall/command と editor選択ダイアログAPIが必要 |
| Req1.3 ダイアログ完了まで設定未確定 | `OpenFileResult::Cancelled` パターン | **Constraint** | 既存 cancel パターンを適用可能、保存タイミング制御が必要 |
| Req2.1 選択パスを設定保存 | `src/settings.rs`, `src/app.rs` | **Missing** | 設定更新関数（外部エディタ専用）と save失敗時ユーザー通知が必要 |
| Req2.2 起動時読込 | `AppController::with_launchers()` | **Covered** | 既に `settings.external_editor` 読込済み |
| Req2.3 再起動後も利用 | `SettingsFile::save/load` | **Covered/Constraint** | 保存成功時は満たす。保存失敗時は現状ユーザー通知なし |
| Req2.4 ダイアログキャンセル時は設定不変 | `OpenFileResult::Cancelled` | **Covered(要接続)** | editor選択ダイアログへ同じ契約を接続すれば成立 |
| Req3.1 設定値を起動先に使用 | `open_in_external_editor()` | **Covered** | 実装済み |
| Req3.2 未設定時 `notepad.exe` | `open_in_external_editor()` | **Covered** | 実装済み |
| Req3.3 1選択1実行ファイル | `ExternalEditorLauncher::launch` | **Covered** | 実装済み |
| Req4.1-4.2 失敗表示+セッション継続 | `show_error_state`, `ViewerError` | **Partially Covered** | 起動失敗は満たす。**設定保存失敗のユーザー通知**は未実装 |

## 3) 主要ギャップと統合課題

1. **設定UIコマンド経路の欠落（Missing）**
   - `External Editor Setting` の action 名、JSイベント、`ViewerCommand` が存在しない。
2. **「エディタ実行ファイル選択」ダイアログAPIの欠落（Missing）**
   - 既存 `FileDialog` は markdown用固定。用途拡張か新traitが必要。
3. **設定保存失敗のユーザー通知欠落（Missing）**
   - 現在はログのみ。要件ではユーザー可視の失敗通知が必要。
4. **境界の整合（Constraint）**
   - product方針「軽量・閲覧中心」を守るため、設定機能は最小導線に限定する必要。

## 4) 実装アプローチ選択肢

### Option A: 既存コンポーネント拡張中心
- **方針**: `FileDialog` trait に editor選択メソッドを追加し、`AppController` に設定更新フローを追加。
- **主な変更先**: `src/platform/windows_file_dialog.rs`, `src/platform/mod.rs`, `src/app.rs`, `src/ui/index.html`, `src/ui/app.js`, `src/sciter/window.rs`。
- **利点**: ファイル追加が少ない。既存パターンに最短で合流。
- **懸念**: `FileDialog` が「Markdown選択 + Editor選択」の多責務化しやすい。

### Option B: 新規コンポーネント分離
- **方針**: editor選択専用 trait/実装（例: `ExternalEditorPicker`）を新設し、`AppController` に注入。
- **主な変更先**: 新規 `src/platform/windows_external_editor_dialog.rs` + wiring。
- **利点**: 責務が明確、テスト分離しやすい。
- **懸念**: 配線点・型パラメータが増え、初期実装量はやや増加。

### Option C: ハイブリッド（推奨候補）
- **方針**: 短期は `FileDialog` へ汎用 `pick_executable_file(...)` を追加して実装、設計で責務境界を明記し将来分離余地を保持。
- **利点**: 速度と保守性のバランスが良い。既存Windowsダイアログ実装を再利用。
- **懸念**: API設計が曖昧だと後で分離コストが増える。

## 5) 複雑度とリスク

- **Effort**: **M (3-7日)**
  - 理由: UI導線追加 + コマンド配線 + Win32ダイアログ拡張 + エラー表示の要件差分対応 + テスト更新が必要。
- **Risk**: **Medium**
  - 理由: 既存パターンは揃っているが、保存失敗時のUX要件追加とダイアログ責務設計で判断点がある。

## 6) Design Phaseへ持ち越す Research Needed

1. **ダイアログフィルタ仕様**（`*.exe`固定か `All Files` 併記か）
2. **保存失敗通知のUI表現**（既存 error area を使うか、別の軽量通知にするか）
3. **非Windows時の扱い**（将来移植時の contract: unavailable error を返す統一方針）
4. **`FileDialog` 拡張 vs 新trait分離の最終判断基準**（責務密度・テスト可読性）

## 7) Designへの推奨入力（決定ではなく論点）

- 優先検討アプローチ: **Option C（ハイブリッド）**
- 先に確定すべき論点:
  1. 新規 `ViewerCommand` 命名とUIイベント経路
  2. Editor選択ダイアログの公開インターフェース
  3. 保存失敗時のユーザー可視エラー契約
  4. 既存外部エディタ起動成功/失敗契約との整合テスト

## 8) Design Synthesis Outcomes

### Generalization
- `pick_markdown_file()` 専用の実装資産を活かしつつ、選択ダイアログを「単一ファイル選択 + Cancelled/Selected 契約」に一般化できる。
- 今回は実装範囲を外部エディタ設定に限定し、一般化はインターフェース形状に留める。

### Build vs Adopt
- **Adopt**: Windows 標準 `GetOpenFileNameW` を再利用（新規ライブラリ非導入）。
- **Rejected**: 新規クロスプラットフォームGUI依存の導入。理由は軽量性方針と依存増大リスク。

### Simplification
- 新規コンポーネント新設より、既存 `FileDialog` / `ViewerCommand` / `AppController` の拡張で最小変更に集約。
- 設定専用画面や履歴管理など将来要素は設計から除外し、現要件達成に必要な最小導線のみ残す。

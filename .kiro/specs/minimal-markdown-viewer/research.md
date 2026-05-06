# Research & Design Decisions: minimal-markdown-viewer

## Summary
- **Feature**: `minimal-markdown-viewer`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 現在の実装は HTML シェル内にも `.titlebar` を持つ一方で、`src/sciter/ffi.rs` の `SciterCreateWindow` には `SW_TITLEBAR | SW_CONTROLS` が残っており、ユーザー視点では二重の上部 UI を作りやすい。
  - `src/ui/app.js` と `src/sciter/window.rs` には既に custom event bridge があり、ファイルオープンと同じ経路でタイトルバー操作を Rust 側へ渡せる。
  - Sciter の DOM `Window` API には `state`、`close()`、`frameType` があるが、既存コードは HWND を Rust 側で保持しているため、Windows ネイティブ操作は Rust 境界に閉じ込めた方がテストしやすく責務も明確である。

## Research Log

### 既存 UI シェルと拡張ポイント
- **Context**: 統合タイトルバー変更が既存 viewer のどこに入るかを確認した。
- **Sources Consulted**: `src/ui/index.html`、`src/ui/styles.css`、`src/ui/app.js`、`src/html_shell.rs`、`src/ui/mod.rs`
- **Findings**:
  - HTML シェルには既に `<header class="titlebar">`、ファイル名領域、open/search/theme/more のコントロールがある。
  - `.titlebar` は通常フロー上の要素であり、本文が長いとページ全体と一緒にスクロールする。
  - `HtmlShell` はローカル asset URL のみを注入し、remote resource を拒否する設計になっている。
- **Implications**: 変更は `HtmlShell` と UI テンプレートの責務に収められる。統合タイトルバー化では remote resource policy を壊さず、UI 構造だけを更新する。

### Sciter ウィンドウ生成とイベントブリッジ
- **Context**: Windows 標準タイトルバー除去とタイトルバー操作の受け口を確認した。
- **Sources Consulted**: `src/sciter/ffi.rs`、`src/sciter/window.rs`、`src/app.rs`
- **Findings**:
  - `SciterApi::create_window()` は `SW_MAIN | SW_TITLEBAR | SW_RESIZEABLE | SW_CONTROLS` を指定している。
  - `ViewerCommand` は `open-file-requested` だけを扱い、`AppController` も open file のみを処理する。
  - `SciterWindow` は event bridge の設置責務を既に持っているため、window chrome 系コマンドをここで横取りしても `AppController` を汚さずに済む。
- **Implications**: 統合タイトルバーでは command routing を二分する。文書系コマンドは従来どおり `AppController`、ウィンドウ系コマンドは `SciterWindow` から Windows host adapter へ渡す。

### Sciter DOM Window API の確認
- **Context**: タイトルバー操作を JS 完結にする選択肢の妥当性を確認した。
- **Sources Consulted**: `https://docs.sciter.com/docs/DOM/Window`
- **Findings**:
  - Sciter `Window` は `state`、`close()`、`frameType` を持ち、caption-less/extended frame を表現できる。
  - ただし現行アプリは Rust 側の `SciterWindowHandle` と User32 FFI を既に保持している。
- **Implications**: JS のみで閉じる案も可能だが、現在のテストと責務分離に合わせるなら Windows 操作は Rust host adapter に集約した方が安全である。

### Windows ネイティブ操作の確認
- **Context**: ドラッグ移動と状態切り替えを最小の Windows API で扱えるかを確認した。
- **Sources Consulted**: `https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nclbuttondown`、`https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow`
- **Findings**:
  - `ShowWindow()` は最小化、最大化、復元の標準状態遷移を提供する。
  - `WM_NCLBUTTONDOWN` は non-client caption drag の OS 標準処理に接続できる。
- **Implications**: 新規 Windows host adapter は HWND に対して `minimize`、`toggle_maximize`、`close`、`begin_drag` を提供するだけで requirements を満たせる。複雑な custom resize hit-test は今回の必須範囲に含めない。

### Markdown / runtime 既存方針の再確認
- **Context**: 統合タイトルバー変更で既存 viewer 責務がぶれないことを確認した。
- **Sources Consulted**: `src/document.rs`、`src/markdown.rs`、`Cargo.toml`、`design/initialdesign.md`
- **Findings**:
  - Markdown 読み込みは read-only の `DocumentLoader` に閉じている。
  - GFM 表示は Comrak 0.52 系の preset で既に管理されている。
  - 新しい dependency は不要で、Rust + Comrak + Sciter.js SDK + User32 の既存前提で完結する。
- **Implications**: design は UI shell / window chrome の拡張に集中し、document pipeline や dependency set を広げない。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 標準タイトルバー維持 + CSS 固定ヘッダー | HTML ヘッダーだけ固定し、Windows 標準タイトルバーは残す | 実装が最小 | ユーザー要求の「単一タイトルバー」を満たさない | 不採用 |
| JS 完結の window 制御 | `Window.this.state` と `close()` を `app.js` から直接呼ぶ | Rust 変更が少ない | host 責務が script に漏れ、既存 Rust test seam を活かしにくい | 不採用 |
| Rust host adapter + 既存 event bridge | HTML は command を発火し、`SciterWindow` が window 操作だけを Rust で処理する | 既存構造に合い、責務と test seam が明確 | Windows 専用 module を 1 つ追加する必要がある | 採用 |

## Design Decisions

### Decision: AppController は文書コマンドだけを持ち続ける
- **Context**: 既存 `AppController` は open file 以外の UI/OS 責務を持っていない。
- **Alternatives Considered**:
  1. すべてのタイトルバー操作を `AppController` に集約する。
  2. すべてを JS に寄せる。
- **Selected Approach**: 文書系イベントだけを `AppController` に流し、window chrome 系イベントは `SciterWindow` が内部で処理する。
- **Rationale**: viewer flow と OS window flow の責務を分けたまま、既存の open-file path を壊さない。
- **Trade-offs**: event bridge で command の振り分けが必要になる。
- **Follow-up**: `src/sciter/window.rs` の tests で app command と window command の振り分けを検証する。

### Decision: 固定タイトルバーはスクロールコンテナ分離で実現する
- **Context**: 現在はページ全体がスクロールし、アイコン帯も一緒に動く。
- **Alternatives Considered**:
  1. body 全体をそのまま使い `position: fixed` で titlebar を上に重ねる。
  2. titlebar と本文 viewport を別コンテナに分ける。
- **Selected Approach**: titlebar を非スクロール領域、error/content を `viewer-viewport` のような単一スクロール領域に分離する。
- **Rationale**: タイトルバー固定と本文スクロールの責務が明確になり、error 表示も titlebar の下に置きやすい。
- **Trade-offs**: `index.html` と `html_shell.rs` の DOM 構造変更が入る。
- **Follow-up**: HTML shell tests で titlebar と viewport の sibling 構造を固定する。

### Decision: Windows 操作は新しい host adapter に閉じ込める
- **Context**: minimize/maximize/close/drag は viewer state ではなく OS window state を変える。
- **Alternatives Considered**:
  1. `sciter/window.rs` に Win32 呼び出しを直接書く。
  2. 新しい Windows 専用 helper に分離する。
- **Selected Approach**: `src/platform/windows_window_chrome.rs` を追加し、`SciterWindow` はそこへ委譲する。
- **Rationale**: User32 依存を platform 層に閉じ込め、Sciter event bridge は routing に専念できる。
- **Trade-offs**: 小さな module は増えるが、OS 固有責務の所在が明確になる。
- **Follow-up**: maximize 状態の判定と toggle 動作を unit tests で固定する。

### Decision: 追加 dependency は導入しない
- **Context**: タイトルバー統合は既存 runtime と OS API だけで完結できる。
- **Alternatives Considered**:
  1. 新しい window chrome crate を導入する。
  2. 既存 Rust + User32 FFI を使う。
- **Selected Approach**: 既存 dependency set のまま、必要な Win32 呼び出しだけを platform module に追加する。
- **Rationale**: lightweight / fast startup / minimal memory の project 方針に合う。
- **Trade-offs**: 少量の FFI を自前管理する必要がある。
- **Follow-up**: unsafe は OS boundary file に限定する。

## Synthesis Outcomes
- 「統合タイトルバー」と「本文スクロール」は別要件に見えて、実際には shell layout の 1 つの責務としてまとめられるため、新しい永続 state は追加しない。
- build-vs-adopt の観点では、Markdown 表示は既存の Comrak/Sciter をそのまま採用し、今回 build するのは Windows host adapter と command routing だけに絞る。
- 将来の search/theme/more は titlebar 上に表示しても、今回は disabled manifest と no-op behavior に限定し、active command model を増やさない。

## Risks & Mitigations
- Windows 標準タイトルバーを隠しても drag が効かないリスク — host adapter に `begin_drag` を持たせ、タイトルバー非ボタン領域からのみ発火させる。
- chrome command と app command の責務混在 — `SciterWindow` で routing し、`AppController` へは open-file だけを渡す。
- shell 変更で local-only policy を壊すリスク — 新しい window icon も `UiAssets` と `ensure_local_only()` を必ず通す。
- `.kiro/steering/` 不在による project-wide 文脈不足 — `design/initialdesign.md` と既存 Rust 実装パターンを優先して設計する。

## References
- [Sciter Window class docs](https://docs.sciter.com/docs/DOM/Window) — `state`、`close()`、`frameType` の確認
- [WM_NCLBUTTONDOWN](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-nclbuttondown) — caption drag 接続の根拠
- [ShowWindow](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow) — 最小化/最大化/復元の標準 API
- [Comrak docs.rs](https://docs.rs/comrak/latest/comrak/) — 既存 GFM renderer の前提
- [Sciter.JS SDK download](https://sciter.com/download/) — runtime / DLL 配布前提の確認

# 調査ログと設計判断

## Summary
- **Feature**: `drag-and-drop-file-open`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存の MDLuma は単一ドキュメント前提であり、現在ウィンドウ内の複数文書保持やタブ状態を持たない。
  - Sciter の公式 DnD では `willacceptdrop` / `drop` と `dataType: "file"`、`data.file` の複数パス受け渡しが定義されており、既存の `window.xcall(name, ...args)` 機構で Rust 側へ引き渡せる。
  - 既存の複数ファイル方針は「2-10 件は別インスタンス起動」であり、ドラッグ＆ドロップでも同じ配分規則を維持するのが最小変更である。

## Research Log

### 既存のファイルオープン責務
- **Context**: ドロップ入力をどこへ接続すべきかを判断するため。
- **Sources Consulted**: `src/app.rs`, `src/lib.rs`, `src/startup_args.rs`, `src/document.rs`, `src/html_shell.rs`
- **Findings**:
  - UI からの通常オープンは `AppController::open_file_requested()` から `open_selected_path()` へ流れる。
  - 起動時は `prepare_startup_path()` が状態だけを準備し、複数ファイルは `LaunchAction::SpawnChildren` へ分岐する。
  - `ViewerState` は `NoDocument` / `DocumentLoaded` / `ErrorVisible` の 3 態だけで、常に 1 文書だけを現在表示として扱う。
  - `DocumentLoader` は渡されたパスを読む責務だけを持ち、フォルダ選別やドロップ形式判定は持たない。
- **Implications**:
  - ドロップ入力も `AppController` の既存オープン経路へ合流させる。
  - フォルダ除外や件数上限制御は `DocumentLoader` ではなく、事前のドロップ配分ポリシーで行う。

### Sciter の DnD とネイティブ橋渡し
- **Context**: Web 標準前提ではなく、Sciter の有効なイベント契約を確認するため。
- **Sources Consulted**: `vendor/sciter-js-sdk-main/docs/md/DOM/Event.md`, `vendor/sciter-js-sdk-main/docs/md/DOM/Window.md`, `src/ui/app.js`, `src/sciter/window.rs`, `src/sciter/ffi.rs`
- **Findings**:
  - Sciter は `willacceptdrop` と `drop` のシステム DnD イベントを提供し、`event.detail.dataType == "file"` のとき `event.detail.data.file` に単一または複数パスを渡す。
  - Sciter の `window.xcall(name, arg0, ..., argN)` は既存の `SciterWindowAttachEventHandler` に接続される。
  - 既存コードは `Window.this.xcall("open-file-requested")` を `ScriptingMethodParams` で受け取り、`ViewerCommand` に正規化している。
  - `SciterLoadHtml` の即時再入は Windows で白画面化リスクがあり、既存コードは可視ウィンドウ更新を遅延ロードへ寄せている。
- **Implications**:
  - 低レベル Win32 の独自 DnD 経路を新設せず、Sciter の DOM DnD と既存 `xcall` 橋を拡張する。
  - Rust 側では `ScriptingMethodParams` の引数読み取りを最小限追加し、ドロップされたパス列だけを取り出す。
  - ドロップ後の文書差し替えは既存の遅延 HTML 更新規則をそのまま使う。

### 子ウィンドウ起動の再利用境界
- **Context**: 2-10 件目をどこで起動するかを決めるため。
- **Sources Consulted**: `src/lib.rs`, `src/startup_args.rs`
- **Findings**:
  - 現在の子インスタンス起動は `lib.rs` の `spawn_children()` に閉じている。
  - 起動時の上限値は `startup_args.rs` の `MAX_FILE_COUNT = 10` に固定されている。
  - `spawn_children()` は実行ファイルの取得と `std::process::Command` 呼び出しを担当しており、`AppController` の責務外である。
- **Implications**:
  - ドロップ入力でも同じ起動方針を再利用できるよう、子ウィンドウ起動責務は専用モジュールへ抽出する。
  - 上限値 10 は CLI とドロップで共有定義に寄せる。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| Sciter DOM DnD + xcall 拡張 | JS で DnD を受け、Rust へ `xcall` 引数でパス列を渡す | 既存 UI 橋を再利用できる、Sciter 公式契約に沿う、Win32 独自処理を増やさない | `SciterValue` 文字列引数の最小デコード追加が必要 | 採用 |
| Sciter Behavior Event 直接解析 | Rust 側で `BehaviorEventParams.data` を直接解釈する | JS 変更を減らせる可能性 | `SciterValue` の複合データ解析がより重く、契約の明示性が低い | 不採用 |
| Win32 独自ドロップ処理 | `WM_DROPFILES` などを直接処理する | パス取得自体は単純 | Sciter 境界を迂回し、Windows 専用責務が増える | 今回は過剰 |

## Design Decisions

### Decision: Sciter の DOM DnD を正規入口にする
- **Context**: 既存 UI 橋を壊さずに通常ファイルドロップを扱いたい。
- **Alternatives Considered**:
  1. Win32 独自ドロップ経路を追加する
  2. Sciter の DOM DnD を受けて `xcall` で Rust へ渡す
- **Selected Approach**: `src/ui/app.js` で `willacceptdrop` と `drop` を購読し、受理したパス列を `Window.this.xcall("open-dropped-files", ...paths)` で Rust 側へ渡す。
- **Rationale**: Sciter の公式契約に沿い、既存の `open-file-requested` 橋を自然に拡張できる。
- **Trade-offs**: `ScriptingMethodParams` の引数デコードが必要になるが、Win32 専用ロジックの追加より境界が明快である。
- **Follow-up**: JS から渡される引数の順序保存と、タイトルバーを含むウィンドウ全体での受理確認をテストする。

### Decision: ドロップ配分は専用ポリシーで決める
- **Context**: フォルダ無視、10 件上限、先頭 1 件だけ現在ウィンドウという規則を CLI とずらさずに保ちたい。
- **Alternatives Considered**:
  1. `AppController` 内で直接フィルタと分配を行う
  2. 小さな共有ポリシーモジュールで分配規則を持つ
- **Selected Approach**: `src/open_paths.rs` を追加し、上限制御とドロップ入力の配分結果を `DropOpenPlan` として返す。
- **Rationale**: 共有ルールの重複を防ぎつつ、`AppController` には「計画されたパスを実行する」責務だけを残せる。
- **Trade-offs**: 新規モジュールは 1 つ増えるが、CLI と DnD の規則差分を追いやすくなる。
- **Follow-up**: `startup_args.rs` の 10 件上限を共有定義へ移し、順序保持の単体テストを追加する。

### Decision: 子ウィンドウ起動は AppController へ直接埋め込まない
- **Context**: 2-10 件目の別インスタンス起動を再利用しつつ、テストしやすくしたい。
- **Alternatives Considered**:
  1. `AppController` から直接 `std::process::Command` を呼ぶ
  2. 起動責務を `ViewerChildLauncher` 境界へ分離する
- **Selected Approach**: `src/viewer_launcher.rs` を追加し、`ViewerChildLauncher` と `ProcessViewerChildLauncher` を定義する。`lib.rs` と `app.rs` はこの境界を共有利用する。
- **Rationale**: 起動時フローとドロップ後の追加起動を 1 つの責務へ寄せられ、テストでは記録用実装へ差し替えられる。
- **Trade-offs**: `StartupController` の型引数が 1 つ増える。
- **Follow-up**: 子起動失敗時は current window を巻き戻さない方針をテストで固定する。

### Decision: 単一ドキュメント状態を維持し、新しい UI 状態は増やさない
- **Context**: この spec はファイル入力経路の追加であり、ビューアー体験全体の再設計ではない。
- **Alternatives Considered**:
  1. 複数ドキュメント状態やタブ状態を導入する
  2. 既存の `ViewerState` とエラー表示を再利用する
- **Selected Approach**: 先頭 1 件だけを現在ウィンドウで `open_selected_path()` 相当の経路へ流し、他は子起動へ回す。
- **Rationale**: 既存要件と `minimal-markdown-viewer` / `command-line-file-open` の境界を壊さない。
- **Trade-offs**: ドロップ専用の高度なフィードバックや集約通知は持たない。
- **Follow-up**: ドロップ中ハイライトや特殊ドロップ対応は別 spec として扱う。

## Risks & Mitigations
- Sciter の DnD イベント名や引数形が想定と異なるリスク — 実装前に vendor docs のイベント契約に沿ったテストを追加し、`willacceptdrop` と `drop` の両方を固定する。
- `SciterValue` 文字列引数の読み取りが不足するリスク — xcall 引数デコードを「文字列配列のみ」に限定し、汎用オブジェクト変換を作らない。
- 子ウィンドウ起動の OS 失敗が current window を壊すリスク — 先頭 1 件の表示更新と追加起動の責務を分離し、起動失敗は current window を巻き戻さない設計にする。

## References
- `vendor/sciter-js-sdk-main/docs/md/DOM/Event.md` — Sciter の drag and drop イベント契約
- `vendor/sciter-js-sdk-main/docs/md/DOM/Window.md` — `window.xcall()` のネイティブ橋渡し契約
- `src/app.rs` — 既存の現在ウィンドウ向けオープンとエラー表示
- `src/lib.rs` — 既存の子インスタンス起動経路
- `src/startup_args.rs` — 既存の 10 件上限と複数ファイル方針

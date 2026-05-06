# 実装済み機能の棚卸し

updated_at: 2026-05-03

spec 駆動開発を導入する前に実装された機能、または spec ファイルを持たずに実装された機能を記録する。
これらはコードベースに存在するが `.kiro/specs/` には対応する spec がない。
新規 spec を設計する際は、既存機能との衝突や重複がないか本ファイルを参照すること。

## Spec ありで実装済みの機能

以下の機能は `.kiro/specs/` に対応する spec を持ち、実装も完了している。

| Spec | 概要 |
|------|------|
| minimal-markdown-viewer | 統合タイトルバー、window chrome、単一文書 viewer shell |
| command-line-file-open | 起動引数からのファイルオープン、複数ファイル時の子起動 |
| document-path-normalization | ドキュメントパスの絶対パス化と保持 |
| drag-and-drop-file-open | ドロップファイルの採用/分配、UI 側 DnD ハンドリング |
| text-selection-copy | 本文のテキスト選択とクリップボードコピー |

## Spec なしで実装済みの機能

### search (Ctrl+F 検索)

- **commit**: `86a96fc`, `95931ab`
- **概要**: Ctrl+F による検索パネル、Sciter Range API でのハイライト、次/前ナビゲーション
- **主な変更箇所**: `src/ui/app.js`, `src/ui/styles.css`, `src/ui/index.html`, `src/ui/mod.rs`, `src/html_shell.rs`, `src/sciter/ffi.rs`, `src/sciter/runtime_assets.rs`
- **追加分**: search icon assets (light/dark), debug log チャネル (`ViewerCommand::DebugLog`), Sciter script runtime feature 設定
- **注意**: spec `minimal-markdown-viewer` の requirements では search は out of scope と明記されているが、実装済み

### sciter-version-check (DLL バージョン検証)

- **commit**: `91ba5b0`, `653c3a2`
- **概要**: Sciter DLL のバージョン不一致を起動前に検出して fast-fail する。major.minor.patch.build まで追跡
- **主な変更箇所**: `src/sciter/ffi.rs`, `src/sciter/runtime.rs`, `src/sciter/generated_sciter_bindings.rs`
- **注意**: `minimal-markdown-viewer` spec の Requirement 7（配布後の起動可能性）に一部対応

### generated-sciter-bindings (bindgen 自動生成 FFI)

- **commit**: `7ef778b`
- **概要**: 手動の API テーブルインデックス解決を廃止し、bindgen 生成の `ISciterAPI` 構造体ベースの FFI に移行
- **主な変更箇所**: `src/sciter/ffi.rs`（大幅縮小）, `src/sciter/generated_sciter_bindings.rs`（新規 +1609行）, `tools/bindgen/`
- **注意**: SDK テーブルレイアウト変更への耐性向上が目的。`tools/bindgen/generate_sciter_bindings.ps1` で再生成可能

### dual-path-drag-and-drop (native DnD 経路)

- **commit**: `84372d0`
- **概要**: Sciter の OLE IDropTarget が DOM 再構築後に壊れる問題に対し、Sciter exchange events と WM_DROPFILES の二重経路で対応
- **主な変更箇所**: `src/sciter/ffi.rs`, `src/sciter/window.rs`, `src/ui/app.js`
- **注意**: spec `drag-and-drop-file-open` は JS 側と policy 側を扱うが、この native 側の exchange/WM_DROPFILES 実装は spec 外

### windows-terminal-hide (コンソール非表示)

- **commit**: `8eada55`
- **概要**: Windows で実行時にコンソールウィンドウを表示しない
- **主な変更箇所**: `src/main.rs`（`#![windows_subsystem = "windows"]`）
- **注意**: 配布ビルドの前提。spec `minimal-markdown-viewer` の Requirement 7.1 に暗黙対応

### wm-app-deferred-load (遅延 HTML ロード)

- **commit**: `d18c2cf`
- **概要**: SciterLoadHtml の遅延メッセージを `WM_APP` レンジに変更し、Sciter 内部メッセージとの衝突を回避
- **主な変更箇所**: `src/sciter/ffi.rs`
- **注意**: Sciter DOM 再構築後の DnD 動作確保に関連

---
このファイルは機能一覧ではなく、spec 外で実装済みの機能が何であるかを把握するための永続メモリである。

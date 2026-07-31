# Roadmap

## Overview

MDLumaのWindows対応を維持したままApple Silicon macOS対応の土台を作る。Sciter.js SDK 6.0.3.18を暫定ベースラインとし、最初にプラットフォーム契約を抽出し、次にSciter共通部からWin32依存を分離する。その後、最小macOS hostを製品経路へ接続し、実際のviewer動作でランタイム互換性を判定する。

このロードマップは`design/macos-porting-architecture.md`のPhase 1、Phase 2と、後続の最小macOS host smokeを対象とする。Phase 0で作成した診断toolkitはtechnical spikeとして保存するが、残作業は移植開始の前提にしない。完全なCocoaアダプター、macOSネイティブフレームUI、`.app`パッケージングは後続フェーズとする。

## Approach Decision

- **Chosen**: migration-firstでPhase 1、Phase 2、最小macOS host smokeを依存順に実施する。Sciter 6.0.3.18は正式採用ではなく暫定baselineとし、製品経路へ接続した後に互換性を判定する。
- **Why**: Phase 0の14 tasksでarchitecture、hash、動的load、API/engine version、限定ABI、popupとwindow lifecycleの基礎証拠は得られた。残るevidence publication基盤を完成させるより、実際のapplication boundaryとruntime boundaryを先に分離し、MDLumaの実利用経路で問題を発見する方が移植の進捗と診断価値を両立できる。
- **Rejected alternatives**: Phase 0の残り19 tasksを移植開始条件として完遂する案は、製品経路から離れた検証基盤へ過剰投資するため採用しない。Phase 1とPhase 2の並行実施は起動構成、URL処理、Sciter window境界の競合リスクが高いため採用しない。6.0.4.8への即時更新はWindows回帰とbindings更新を構造分離へ混在させるため、実アプリsmokeで6.0.3.18の不適合が確認された場合の代替とする。

## Scope

- **In**: プラットフォーム契約、設定・ログのパス解決境界、起動composition root、Sciterローダー境界、共通C APIテーブル、Win32メッセージ処理の隔離、最小macOS loader/window adapter、Markdown viewerの起動・描画・popup・終了smoke、Windows回帰検証。
- **Out**: 完全なCocoaダイアログ等のmacOS実装、macOS向け外部エディタ実装、ネイティブフレームUI、永続的なウィンドウgeometry、`.app`作成、署名、notarization、配布許諾判定、Intel macOS対応、Markdown編集機能。

## Constraints

- 単一Rust crateを維持し、軽量性、起動速度、省メモリ、実装の単純さを優先する。
- MarkdownからHTMLへの変換はComrak、UI描画はSciter.js SDKを維持する。
- Windows 10/11の既存動作を回帰させない。
- 最初のmacOSターゲットは`aarch64-apple-darwin`、最低OSは使用するdylibが要求するmacOS 11.5とする。
- 暫定ランタイムは公式commit `e31ec0f726bdbe5d0402ad647f3b34feef84654e`のSciter 6.0.3.18、API version 10、dylib SHA-256 `be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b`とする。
- 最小macOS host smokeでpopup AV、終了時AV、ABI不整合またはarm64非対応が確認された場合は、Windows DLL、macOS dylib、ヘッダー、bindingsを同時に6.0.4.8へ更新して再検証する。
- dylibの再配布・再署名許諾は`.app`配布前のrelease gateとし、構造分離とローカル実行をblockしない。
- Sciter固有の判断は公式SDK docs、samples、headersを根拠にし、WebViewやブラウザの挙動を前提にしない。

## Boundary Strategy

- **Why this split**: アプリケーションのOS契約、Sciter FFI内部構造、実runtime接続を別々のレビュー単位にすることで、Windows回帰範囲を限定しながら実利用経路へ早く到達する。Phase 1はアプリケーション境界、Phase 2はランタイム境界、host smokeはmacOS接続と暫定runtime判定を所有する。
- **Shared seams to watch**: `src/lib.rs`のcomposition root、`src/platform/mod.rs`の公開面、`src/sciter/ffi.rs`と`src/sciter/window.rs`のopaque window handle、外部URLルーティング、DnD、設定パス、ランタイム名と診断メッセージ。

## Specs (dependency order)

- [~] macos-sciter-runtime-evidence -- 14/33 tasksでtechnical spikeを完了し、残作業をsuspendした。後続仕様をblockしない。Dependencies: none
- [ ] platform-contract-extraction -- OS固有サービスを共有アプリケーションロジックから契約として分離し、Windows動作を維持する。Dependencies: none
- [ ] sciter-win32-separation -- 共通Sciter C API・window処理から動的ローダーとWin32回避策を分離する。Dependencies: platform-contract-extraction
- [ ] macos-sciter-host-smoke -- 最小macOS loader/window adapterを製品経路へ接続し、viewer描画、popup、終了処理と暫定runtime互換性を検証する。Dependencies: platform-contract-extraction, sciter-win32-separation

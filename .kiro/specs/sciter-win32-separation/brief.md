# Brief: sciter-win32-separation

## Problem

macOS向けSciter接続を実装する開発者は、`src/sciter/ffi.rs`と`window.rs`に共通C API、Windows DLL loader、WndProc subclass、メッセージfallback、custom frame、DnD、geometry処理が混在しているため、共通経路を安全に再利用できない。非Windowsでは共通APIメンバーまでplaceholder errorや`cfg(windows)`に遮られている。

## Current State

`SciterApi`は`LoadLibraryW`を内包し、生成済み`ISciterAPI` bindingsの利用と多くの関数ポインタがWindows限定になっている。`SciterWindow`はdeferred HTML loading、`WM_DROPFILES`、keyboard fallback、wheel normalization、window placement、custom frame処理へ直接依存する。一方、Sciter API table lookup、window作成、HTML loading、event handler、DOM command、exchange DnDは両OSで共有可能な責務である。

## Desired Outcome

共通Sciter API/window層がOS固有loaderと小さなplatform window adapterにだけ依存し、すべてのWndProc・Win32 message workaroundがWindows専用モジュールへ隔離される。Windows動作を維持しながら、後続仕様がmacOS loaderとwindow adapterを追加できる明確な接続点ができる。

## Approach

動的ライブラリhandleと`SciterAPI` export解決を`sciter::loader`境界へ抽出し、Windows実装を`loader/windows.rs`へ移す。API tableの必須関数検証と共通wrapperをloaderから分離し、Win32 WndProcとmessage処理を`sciter/windows.rs`へ移す。`SciterWindow`が必要とするOS固有操作だけを小さな内部adapterとして表現し、DOM/xcall/exchange DnDの共通経路はadapter外に保つ。

## Scope

- **In**: loader interfaceとWindows loader、runtime-neutralな名称・診断、共通`SciterApi` table validation、生成bindingsの共通ABI利用範囲、Win32 WndProc/message/custom-frame/DnD fallback/geometry処理の移動、必要最小限のplatform window adapter、DOM commandとexchange DnDのWindows非依存化、Windows回帰テストとmacOS target compile check。
- **Out**: 完成したmacOS loader、Cocoa window adapter、macOS geometry、native-frame HTML/CSS、`.app` packaging、Windows workaroundの削除、Sciter runtime version upgrade。

## Boundary Candidates

- runtime pathから`SciterAPI` exportだけを解決する`sciter::loader`と、API tableを検証・利用する共通wrapper。
- WndProc lifecycleとWin32 messagesを所有する`sciter::windows`。
- common `SciterWindow`が必要な遅延load、event loop、native fallback、placement操作だけを表す内部platform adapter。

## Out of Boundary

- macOSへWin32 workaroundのno-opコピーを作らない。
- DnDの共有経路を`WM_DROPFILES`へ依存させない。
- generated `ISciterAPI`のABI互換性をバージョン番号だけで仮定しない。
- Phase 0のsmokeが成功しても、製品loader未実装のまま非Windows placeholderを無条件に削除しない。

## Upstream / Downstream

- **Upstream**: `platform-contract-extraction`のURL openingとcomposition root境界。`macos-sciter-runtime-evidence`のABI・version観測は参考情報として利用できるが、完了は依存条件ではない。
- **Downstream**: `macos-sciter-host-smoke`、Cocoa platform adapters、native-frame UI、macOS packaging。

## Existing Spec Touchpoints

- **Extends**: なし。既存仕様のSciter内部実装を分離する。
- **Adjacent**: `minimal-markdown-viewer`のwindow/runtime、`drag-and-drop-file-open`のDOM/exchange DnDとWindows fallback、`text-selection-copy`、`theme-toggle`、`font-settings`のDOM command経路。受け入れ動作を変更しない。

## Constraints

Sciter.js SDKの公式headersをABIの根拠とし、WindowsとmacOSでC API tableの同じ必須slotを検証できる構造にする。`unsafe`とOS APIを小さな境界へ限定する。Windowsのdeferred load、DnD fallback、shortcut、wheel、geometry、title-bar workaroundは既存挙動と順序を維持する。構造分離中はSciter 6.0.3.18を暫定baselineとし、正式な互換性判定と必要時のversion更新は`macos-sciter-host-smoke`で行う。

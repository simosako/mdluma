# Brief: platform-contract-extraction

## Problem

macOS対応を追加する開発者は、共有アプリケーションロジックとWindows固有サービスが同じ公開面や起動構成に混在しているため、macOS実装を追加すると`AppController`や共有処理へOS条件分岐を持ち込む危険がある。設定・ログの保存先やブラウザ・既定アプリ起動もWindows前提のままである。

## Current State

`FileDialog`と`WindowChromeController`はWindows実装ファイル内にあり、`src/lib.rs`の起動型は`WindowsFileDialog`と`WindowsFontDialog`を直接指定している。ブラウザ起動はWindows自由関数、設定とデバッグログは`LOCALAPPDATA`を直接参照し、既定外部エディタはWindows前提である。一方、`AppController`は既にジェネリックDIとフェイクを用いたテストを持つ。

## Desired Outcome

共有アプリケーションロジックが標準ライブラリ型またはプロジェクト型だけで定義されたプラットフォーム契約へ依存し、OS実装の選択がcomposition rootだけに限定される。Windows 10/11の既存ダイアログ、設定、ログ、ブラウザ、ウィンドウ操作、外部エディタ動作が変わらない。

本仕様はmigration-firstロードマップの最初の実装specであり、`macos-sciter-runtime-evidence`の完了または正式Go判定を前提としない。

## Approach

単一crateを維持し、`src/platform/contracts.rs`へ必要最小限の契約と共有結果型を集約する。既存Windows実装を`src/platform/windows/`へ段階的に整理し、設定・ログへパスresolverを注入する。`src/lib.rs`にはプラットフォーム選択を所有するcomposition rootを設け、共有aliasや`AppController`からWindows型名を除く。

## Scope

- **In**: `FileDialog`、`OpenFileResult`、`FontDialog`、`FontDialogResult`、必要な`WindowChromeController`状態型、URL opening、application-data/log path、default document openingの契約、Windows実装の適合、設定・ログへのresolver導入、composition root、契約フェイクとWindows回帰テスト。
- **Out**: Cocoa実装、macOS固有パス実装、macOS外部エディタ選択、Sciter loader分離、UI frame variant、既存設定形式の不必要な変更。

## Boundary Candidates

- OS型を公開しない`platform::contracts`と、`platform::windows`配下の具体実装。
- shared controllerから独立したplatform-selected composition root。
- filesystem writingから独立してテストできるapplication-data/log directory resolver。

## Out of Boundary

- `AppController`へ`cfg(windows)`または`cfg(target_os = "macos")`を追加しない。
- `HWND`、Cocoa型、Sciter native handleを共有契約の値型にしない。
- macOS挙動を未実装のno-opで偽装しない。
- 既存ユーザー機能の要件や設定データを必要なく変更しない。

## Upstream / Downstream

- **Upstream**: 既存ジェネリックDI、Windows platform implementations。`macos-sciter-runtime-evidence`の取得済み知見は参照できるが、依存条件ではない。
- **Downstream**: `sciter-win32-separation`、macOS file/font dialog・browser・paths・default opener adapters。

## Existing Spec Touchpoints

- **Extends**: なし。既存仕様の機能動作を保つ内部アーキテクチャ変更である。
- **Adjacent**: `minimal-markdown-viewer`のOpenとウィンドウ操作、`font-settings`のネイティブフォントダイアログ、`external-editor-integration`の設定・起動先、`about-dialog`と外部リンク。これらの受け入れ動作を変更しない。

## Constraints

Windowsを既定ターゲットとして維持し、既存テストとWindowsビルドを継続して通す。抽象化はmacOS adapterに必要な責務だけへ限定し、AppControllerのジェネリック複雑性を不必要に増やさない。設定・ログのパス解決失敗時は既存の一時ディレクトリfallbackを維持する。Sciter runtimeの正式採用判断やversion更新は本仕様へ混在させない。

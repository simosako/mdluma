# Brief: macos-sciter-host-smoke

## Problem

プラットフォーム契約とSciter Win32依存を分離した後も、macOSの製品経路でMarkdown viewerが起動・描画でき、MDLumaが使うpopupと終了処理が安定するかは未確認である。standalone harnessだけではcomposition root、HTML resource、event loop、window lifecycleを含む実アプリ互換性を判断できない。

## Current State

`macos-sciter-runtime-evidence`のtechnical spikeにより、公式Sciter 6.0.3.18 dylibのApple Silicon slice、固定hash、load、API version 10、engine version、限定ABI、popupとwindow lifecycleの基礎観測が得られている。`platform-contract-extraction`と`sciter-win32-separation`が本仕様の接続境界を提供する予定である。

## Desired Outcome

最小macOS loaderとwindow adapterをMDLumaの製品経路へ接続し、Apple Silicon上でMarkdown文書の起動・HTML描画、基本操作、popup、正常終了を再現可能に確認できる。Sciter 6.0.3.18を継続するか、両OS runtime・headers・bindingsを6.0.4.8へ一括更新するかを実アプリの証拠で決定できる。

## Approach

Phase 1とPhase 2が提供するcomposition root、loader、platform window境界へ、起動に必要な最小macOS実装だけを追加する。既存fixtureとdiagnostic toolkitは必要な範囲で再利用し、製品のMarkdown-to-HTML、resource load、event loop、popup、close経路をsmoke testする。6.0.3.18固有の不適合が確認された場合だけ、Windows DLL、macOS dylib、公式headers、生成bindingsを同一revisionの6.0.4.8へ更新して再検証する。

## Scope

- **In**: macOS `dlopen` loader、最小window adapter、composition root接続、Markdown viewer起動とHTML描画、popupとwindow close smoke、runtime/version診断、Windows回帰確認、必要時の6.0.4.8一括更新判断。
- **Out**: 完全なCocoa file/font dialog、外部エディタ統合、native custom frame、永続的なwindow geometry、`.app` packaging、Developer ID署名、notarization、dylib配布・再署名許諾の法的判断、Intel macOS対応。

## Boundary Candidates

- 共通loader契約へ適合するmacOS `dlopen`実装。
- 共通`SciterWindow`が要求する最小macOS event loop・native handle・close操作。
- standalone evidenceではなく製品composition rootから起動する再現可能なsmoke entry。

## Out of Boundary

- macOS未実装サービスを成功するno-opで偽装しない。
- 6.0.3.18と6.0.4.8のruntime、headers、bindingsを混在させない。
- runtime不具合を根拠なくMDLuma固有workaroundで隠さない。
- release許諾未解決をローカル開発用runtime互換性の失敗として扱わない。

## Upstream / Downstream

- **Upstream**: `platform-contract-extraction`、`sciter-win32-separation`、`macos-sciter-runtime-evidence`の取得済みdiagnostic assets。
- **Downstream**: 完全なmacOS platform adapters、native-frame UI、`.app` packaging、署名・notarization、配布release gate。

## Existing Spec Touchpoints

- **Extends**: `minimal-markdown-viewer`の起動、Markdown描画、window lifecycleをmacOSの最小経路へ拡張する。
- **Adjacent**: `about-dialog`のpopup、`theme-toggle`、`text-selection-copy`、`drag-and-drop-file-open`。最初のsmokeはviewer起動に不可欠な経路へ限定する。

## Constraints

最初のtargetは`aarch64-apple-darwin`、最低OSは暫定dylibが要求するmacOS 11.5とする。単一Rust crate、Comrak、Sciter.js SDKを維持し、Windows 10/11の既存動作を回帰させない。Sciter固有実装は公式SDK docs、samples、headersに基づく。再配布・再署名許諾は`.app`配布前のrelease gateとし、本仕様のローカル実行をblockしない。

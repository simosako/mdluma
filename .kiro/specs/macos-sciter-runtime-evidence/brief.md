# Brief: macos-sciter-runtime-evidence

## Status

2026-07-28に14/33 tasks完了地点で実装をsuspendした。作成済みtoolkitと取得済み証拠はtechnical spikeおよび後続の診断資産として保存する。未完了taskは`platform-contract-extraction`、`sciter-win32-separation`、`macos-sciter-host-smoke`をblockしない。

## Problem

macOS移植を進める開発者は、採用予定のSciterランタイムがApple Siliconでロード可能か、既存bindingsとABI互換か、MDLumaで使うpopupや終了処理が安定しているか、同梱再配布できるかを確証できていない。この状態でプラットフォーム実装を始めると、後からランタイム更新と大規模な手戻りが発生する。

## Current State

Windowsでは公式Sciter 6.0.3.18の`sciter.dll`とAPI version 10の生成済みbindingsを使用している。公式commit `e31ec0f726bdbe5d0402ad647f3b34feef84654e`のmacOS `libsciter.dylib`は`vendor/sciter-js-sdk-main/bin/macosx/`へ配置済みで、既知SHA-256と一致し、`x86_64`と`arm64`を含む。製品コードのmacOSローダー、ABI smoke、popup・終了処理の実機検証、再配布条件の確定は未完了である。

## Desired Outcome

Sciter 6.0.3.18の由来、architecture、hash、load、version、限定ABI、popupとwindow lifecycleに関する取得済み証拠を、後続の製品経路smokeで再利用できる。正式なruntime判定は`macos-sciter-host-smoke`が所有する。

## Approach

固定commitとハッシュのmanifest、Apple Silicon上の`dlopen`、`dlsym("SciterAPI")`、API version 10、engine version 6.0.3.18、限定ABI、popupとwindow lifecycleの最小smokeまでをtechnical spikeとして維持する。未実装のsupervisor、evidence store、decision publicationは現時点では追加しない。

## Scope

- **In**: 実装済みのSDK revisionとruntime hash固定、arm64確認、動的loadとAPI/version smoke、限定ABI確認、popupとwindow lifecycle smokeの保存・保守。
- **Out**: 未完了のevidence orchestration・publication、正式Go/No-Go判定、製品コードへのmacOS loader実装、Cocoa UI、`.app` packaging、Developer ID署名、notarization、Windows runtime更新。

## Boundary Candidates

- 外部成果物の同一性・architecture・依存関係を記録するruntime evidence manifest。
- 製品ローダーへ依存しない最小`SciterAPI`ロードsmokeと、MDLuma利用機能に絞った手動安定性チェック。

## Out of Boundary

- Phase 1以降の構造リファクタリングをこの仕様へ混在させない。
- popup不具合をMDLuma側の推測的workaroundで隠さない。
- 法的判断を技術テストの成功だけで代替しない。

## Upstream / Downstream

- **Upstream**: `design/macos-porting-architecture.md`、公式Sciter.js SDK 6.0.3.18 commit、既存Windows runtimeと生成済みbindings。
- **Downstream**: `macos-sciter-host-smoke`が診断toolkitと観測結果を必要に応じて再利用する。`platform-contract-extraction`と`sciter-win32-separation`は本仕様の完了に依存しない。

## Existing Spec Touchpoints

- **Extends**: なし。
- **Adjacent**: `minimal-markdown-viewer`の実行時ファイル・起動失敗要件、`about-dialog`のSciter表示要件。既存のユーザー機能要件は変更しない。

## Constraints

診断対象はSciter 6.0.3.18、API version 10、Apple Siliconとする。再配布・再署名条件は`.app`配布前のrelease gateで解決し、このtechnical spikeや後続のローカル実行をblockしない。

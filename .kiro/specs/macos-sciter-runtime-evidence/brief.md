# Brief: macos-sciter-runtime-evidence

## Problem

macOS移植を進める開発者は、採用予定のSciterランタイムがApple Siliconでロード可能か、既存bindingsとABI互換か、MDLumaで使うpopupや終了処理が安定しているか、同梱再配布できるかを確証できていない。この状態でプラットフォーム実装を始めると、後からランタイム更新と大規模な手戻りが発生する。

## Current State

Windowsでは公式Sciter 6.0.3.18の`sciter.dll`とAPI version 10の生成済みbindingsを使用している。公式commit `e31ec0f726bdbe5d0402ad647f3b34feef84654e`のmacOS `libsciter.dylib`は`vendor/sciter-js-sdk-main/bin/macosx/`へ配置済みで、既知SHA-256と一致し、`x86_64`と`arm64`を含む。製品コードのmacOSローダー、ABI smoke、popup・終了処理の実機検証、再配布条件の確定は未完了である。

## Desired Outcome

Sciter 6.0.3.18をmacOS移植のベースラインとして採用できるかを、再実行可能な検証と記録に基づいてGo/No-Go判定できる。No-Goの場合は6.0.4.8へ両OSランタイムとbindingsを同時更新する条件が明確になっている。

## Approach

固定commitとハッシュをmanifestへ記録し、Apple Silicon上で絶対パス指定の`dlopen`、`dlsym("SciterAPI")`、API version 10、engine version 6.0.3.18を検証する最小smokeを用意する。併せてarchitecture、minimum OS、依存、install name、署名状態、popup反復操作、ウィンドウ終了反復、ライセンス・表示・再配布条件をチェックリスト化し、Go/No-Go結果を残す。

## Scope

- **In**: SDK revisionとruntime hashの固定、arm64確認、動的ロードとAPI/version smoke、既存bindingsのABI境界確認、popupと終了処理の実機ストレス確認、ライセンス・About表記・dylib再配布条件の記録、Go/No-Go判定。
- **Out**: 製品コードへのmacOSローダー実装、Cocoa UI、`.app`パッケージング、Developer ID署名、notarization、Windowsランタイム更新。ただしNo-Go時の更新方針は記録する。

## Boundary Candidates

- 外部成果物の同一性・architecture・依存関係を記録するruntime evidence manifest。
- 製品ローダーへ依存しない最小`SciterAPI`ロードsmokeと、MDLuma利用機能に絞った手動安定性チェック。

## Out of Boundary

- Phase 1以降の構造リファクタリングをこの仕様へ混在させない。
- popup不具合をMDLuma側の推測的workaroundで隠さない。
- 法的判断を技術テストの成功だけで代替しない。

## Upstream / Downstream

- **Upstream**: `design/macos-porting-architecture.md`、公式Sciter.js SDK 6.0.3.18 commit、既存Windows runtimeと生成済みbindings。
- **Downstream**: `platform-contract-extraction`、`sciter-win32-separation`、将来のmacOS adapters、native-frame UI、`.app` packaging。

## Existing Spec Touchpoints

- **Extends**: なし。
- **Adjacent**: `minimal-markdown-viewer`の実行時ファイル・起動失敗要件、`about-dialog`のSciter表示要件。既存のユーザー機能要件は変更しない。

## Constraints

初期ベースラインはSciter 6.0.3.18、API version 10、Apple Siliconとする。6.0.3.18以降でmacOS popup AV、終了時AV、heap corruptionの修正履歴があるため、MDLumaが使用する`menu.popup`と終了処理の実機確認を必須とする。再配布条件に未解決事項が残る場合はGoとしない。

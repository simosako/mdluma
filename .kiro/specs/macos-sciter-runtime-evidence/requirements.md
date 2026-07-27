# Requirements Document

## Introduction

本仕様は、macOS移植を進める開発者がSciter.js SDK 6.0.3.18のmacOS runtimeをApple Silicon向けMDLumaのベースラインとして採用できるか判断するための検証証拠を定義する。現在は公式runtimeの配置、ハッシュ、含有architectureまで確認済みだが、APIと限定的なABI smoke、MDLumaが利用する`menu.popup`とwindow終了処理の安定性、同梱再配布条件が未確定である。再実行可能な検証結果と明示的なGo/No-Go判定を残し、後続のmacOS対応を未検証の前提で開始しない状態を目指す。

## Boundary Context

- **In scope**: 固定したSDK revisionとruntimeの同一性、検証時点の現行Apple Silicon macOSにおけるnative arm64適合性、runtime API互換性、既存bindingsによるversion取得範囲のABI smoke、`menu.popup`とwindow終了処理の実機安定性、ライセンス・About表示・再配布・再署名条件、再実行可能な証拠、Go/No-Go判定。
- **Out of scope**: 未実行の過去macOS versionを含むOS version別compatibility matrix、全Sciter API entryのABI保証、製品コードのmacOS loader、Cocoa UI、`.app` packaging、Developer ID署名、notarization、Phase 1以降の構造変更、No-Go後のruntime更新作業。
- **Adjacent expectations**: 全API entryのABI検証範囲は`sciter-win32-separation`で確定する。Go判定は`platform-contract-extraction`以降の開始条件とする。No-Goの場合はWindows DLL、macOS dylib、対応headers、生成済みbindingsを同一のSciter 6.0.4.8 revisionへ更新する別作業を開始し、本仕様と同じ検証を再実行する。

## Requirements

### Requirement 1: Runtime成果物の同一性と由来
**Objective:** As a macOS移植開発者, I want 検証対象runtimeの由来と同一性を一意に確認したい, so that 意図しない版や改変された成果物を評価対象にしないで済む

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 取得元を公式repository `https://gitlab.com/sciter-engine/sciter-js-sdk`として記録する
2. The macOS Runtime Evidence shall 検証対象revisionをcommit `e31ec0f726bdbe5d0402ad647f3b34feef84654e`として記録する
3. The macOS Runtime Evidence shall 検証対象ファイルを`bin/macosx/libsciter.dylib`として記録する
4. The macOS Runtime Evidence shall 検証対象runtimeの期待SHA-256を`be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b`として記録する
5. When runtime同一性検証を実行する, the macOS Runtime Evidence shall 実際のSHA-256と期待SHA-256の比較結果を記録する
6. If 実際のSHA-256が期待SHA-256と一致しない, then the macOS Runtime Evidence shall runtime同一性を不合格とする
7. If 取得元またはrevisionを確認できない, then the macOS Runtime Evidence shall runtime同一性を不合格とする

### Requirement 2: Apple Silicon実行条件の証拠
**Objective:** As a macOS移植開発者, I want runtimeの対象architectureとmacOS実行条件を確認したい, so that 対象環境で利用できないruntimeを採用しないで済む

#### Acceptance Criteria
1. When binary適合性検証を実行する, the macOS Runtime Evidence shall runtimeに含まれるすべてのarchitectureを記録する
2. If runtimeに`arm64`が含まれない, then the macOS Runtime Evidence shall binary適合性を不合格とする
3. When runtime smoke verificationを実行する, the macOS Runtime Evidence shall 検証processの実行architectureを記録する
4. If runtime smoke verificationがnative `arm64` processとして実行されていない, then the macOS Runtime Evidence shall Apple Silicon実行検証を不合格とする
5. The macOS Runtime Evidence shall runtimeが要求するminimum macOS versionを記録する
6. The macOS Runtime Evidence shall runtimeの外部依存を記録する
7. The macOS Runtime Evidence shall runtimeのinstall nameを記録する
8. The macOS Runtime Evidence shall runtimeの署名状態を記録する

### Requirement 3: Runtime APIの互換性
**Objective:** As a macOS移植開発者, I want runtimeが期待するSciter APIを提供することを確認したい, so that 後続実装が異なるAPI versionへ接続しないで済む

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 期待engine versionを`6.0.3.18`として記録する
2. The macOS Runtime Evidence shall 期待API versionを`10`として記録する
3. When native arm64 runtime smoke verificationを実行する, the macOS Runtime Evidence shall 指定した検証対象runtimeの読み込み結果を記録する
4. When runtimeの読み込みに成功する, the macOS Runtime Evidence shall `SciterAPI` exportの解決結果を記録する
5. When `SciterAPI` exportの解決に成功する, the macOS Runtime Evidence shall runtimeが返すAPI tableがnullでないことを確認する
6. When runtimeからengine versionを取得する, the macOS Runtime Evidence shall 実際のengine versionと期待engine versionの比較結果を記録する
7. When runtimeからAPI versionを取得する, the macOS Runtime Evidence shall 実際のAPI versionと期待API versionの比較結果を記録する
8. If runtimeの読み込み、`SciterAPI` exportの解決、またはAPI tableの取得に失敗する, then the macOS Runtime Evidence shall runtime API互換性を不合格とする
9. If 実際のengine versionまたはAPI versionが期待値と一致しない, then the macOS Runtime Evidence shall runtime API互換性を不合格とする

### Requirement 4: 既存BindingsによるABI Smoke
**Objective:** As a macOS移植開発者, I want 既存bindingsがversion取得に必要なABI境界を利用できることを確認したい, so that 最初の共通Sciter分離を未検証のAPI table先頭layoutへ依存させないで済む

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 既存bindingsのengine version定数と同一revisionの公式headersのengine version定数を比較する
2. The macOS Runtime Evidence shall 既存bindingsのAPI version定数と同一revisionの公式headersのAPI version定数を比較する
3. When 既存bindingsのAPI table型を用いたnative arm64 ABI smokeを実行する, the macOS Runtime Evidence shall API version fieldの取得結果を記録する
4. When 既存bindingsのAPI table型を用いたnative arm64 ABI smokeを実行する, the macOS Runtime Evidence shall `SciterVersion` entryの取得結果を記録する
5. When 既存bindingsの`SciterVersion` entryを呼び出す, the macOS Runtime Evidence shall processが正常終了したかを記録する
6. When 既存bindingsの`SciterVersion` entryを呼び出す, the macOS Runtime Evidence shall 取得したengine versionを記録する
7. If bindingsと公式headersのengine version定数またはAPI version定数が一致しない, then the macOS Runtime Evidence shall ABI smokeを不合格とする
8. If API version fieldまたは`SciterVersion` entryを取得できない, then the macOS Runtime Evidence shall ABI smokeを不合格とする
9. If bindings経由の呼び出しが異常終了するか期待engine versionを返さない, then the macOS Runtime Evidence shall ABI smokeを不合格とする
10. The macOS Runtime Evidence shall version取得以外のAPI entryを本ABI smokeの検証済み範囲に含めない

### Requirement 5: PopupとWindow終了処理の実機安定性
**Objective:** As a macOS移植開発者, I want MDLumaが利用するSciter操作の安定性を実機で確認したい, so that 既知のmacOS crashリスクを見落としたまま採用しないで済む

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 実機安定性検証を検証時点の現行Apple Silicon macOS上のnative arm64 processで実行する
2. When popup安定性検証を実行する, the macOS Runtime Evidence shall `menu.popup`の表示と終了を100回連続で完了する
3. When window終了安定性検証を実行する, the macOS Runtime Evidence shall Sciter windowの生成と終了を100回連続で完了する
4. The macOS Runtime Evidence shall 各popupサイクルの表示完了と終了完了を記録する
5. The macOS Runtime Evidence shall 各windowサイクルの生成完了と終了完了を記録する
6. If popupサイクルが10秒以内に完了しない, then the macOS Runtime Evidence shall popup安定性検証を不合格とする
7. If windowサイクルが10秒以内に完了しない, then the macOS Runtime Evidence shall window終了安定性検証を不合格とする
8. If 検証中にcrash、異常終了、access violation、またはheap corruptionを検出する, then the macOS Runtime Evidence shall 該当する実機安定性検証を不合格とする
9. If 実機安定性検証が不合格になる, then the macOS Runtime Evidence shall 失敗したサイクルと診断情報を記録する
10. The macOS Runtime Evidence shall Go判定の実機安定性証拠を記録された実行macOS versionに限定し、未実行のmacOS versionを検証済みとして扱わない

### Requirement 6: ライセンスと再配布条件の確認
**Objective:** As a 配布責任者, I want macOS runtimeの利用・表示・再配布条件を確認したい, so that 権利条件が未解決または遵守不能な成果物をMDLumaへ同梱しないで済む

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 検証対象revisionの公式`LICENSE`を識別する
2. The macOS Runtime Evidence shall 検証対象revisionの公式`SCITER-ENGINE-EULA.md`を識別する
3. The macOS Runtime Evidence shall 配布物へ同梱する必要があるライセンス文書を記録する
4. The macOS Runtime Evidence shall MDLumaのAbout表示に必要なSciter表記を記録する
5. The macOS Runtime Evidence shall `libsciter.dylib`の同梱再配布可否をSciter提供元の公開文書または書面回答に基づいて記録する
6. The macOS Runtime Evidence shall `libsciter.dylib`の再署名可否をSciter提供元の公開文書または書面回答に基づいて記録する
7. If 同梱再配布可否を確定できない, then the macOS Runtime Evidence shall ライセンス確認を不合格とする
8. If `libsciter.dylib`の同梱再配布が許可されない, then the macOS Runtime Evidence shall ライセンス確認を不合格とする
9. If 必須ライセンス文書またはAbout表示要件を確定できない, then the macOS Runtime Evidence shall ライセンス確認を不合格とする
10. If 再署名可否を確定できない, then the macOS Runtime Evidence shall ライセンス確認を不合格とする
11. If `libsciter.dylib`の再署名が許可されない, then the macOS Runtime Evidence shall ライセンス確認を不合格とする

### Requirement 7: 再実行可能な証拠記録
**Objective:** As a macOS移植のレビュー担当者, I want 検証条件と結果を再現・比較できる形で確認したい, so that 報告者の判断だけに依存せず結果を評価できる

#### Acceptance Criteria
1. The macOS Runtime Evidence shall 各検証の実行日時を記録する
2. The macOS Runtime Evidence shall 各検証を実行したhardwareを記録する
3. The macOS Runtime Evidence shall 各検証を実行したmacOS versionを記録する
4. The macOS Runtime Evidence shall 各検証processのarchitectureを記録する
5. The macOS Runtime Evidence shall 各検証で使用したruntimeのパスを記録する
6. The macOS Runtime Evidence shall 各検証で使用したruntimeのSHA-256を記録する
7. The macOS Runtime Evidence shall 各検証の実行手順を記録する
8. The macOS Runtime Evidence shall 各検証の終了状態を記録する
9. The macOS Runtime Evidence shall 各検証結果を合格、不合格、または未実施のいずれかで記録する
10. When 検証を再実行する, the macOS Runtime Evidence shall 前回記録を保持したまま新しい結果を追加する
11. If 検証が不合格になる, then the macOS Runtime Evidence shall 失敗段階を記録する
12. If 検証が不合格になる, then the macOS Runtime Evidence shall 取得できた診断情報を記録する

### Requirement 8: Go/No-Go判定と後続作業の制御
**Objective:** As a macOS移植の意思決定者, I want 全検証結果と判断理由を一つの判定として確認したい, so that 後続フェーズへ進めるかを一貫して判断できる

#### Acceptance Criteria
1. The macOS Runtime Evidence shall Requirements 1から7を必須判定領域として扱う
2. When Requirements 1から7のすべてのAcceptance Criteriaを満たす, the macOS Runtime Evidence shall Sciter 6.0.3.18の判定をGoとして記録する
3. If Requirements 1から7に不合格または未実施のAcceptance Criteriaが一つでもある, then the macOS Runtime Evidence shall 判定をNo-Goとして記録する
4. If 判定がNo-Goである, then the macOS Runtime Evidence shall 各No-Go理由を記録する
5. If 判定がNo-Goである, then the macOS Runtime Evidence shall `platform-contract-extraction`をBlockedとして記録する
6. If 判定がNo-Goである, then the macOS Runtime Evidence shall `sciter-win32-separation`をBlockedとして記録する
7. If 判定が未確定である, then the macOS Runtime Evidence shall `platform-contract-extraction`をBlockedとして記録する
8. If 判定が未確定である, then the macOS Runtime Evidence shall `sciter-win32-separation`をBlockedとして記録する
9. When 判定がGoになる, the macOS Runtime Evidence shall `platform-contract-extraction`のruntime evidence依存を充足として記録する
10. If 判定がNo-Goである, then the macOS Runtime Evidence shall Windows DLL、macOS dylib、対応headers、および生成済みbindingsを同一のSciter 6.0.4.8 revisionへ更新する方針を記録する
11. The macOS Runtime Evidence shall No-Go後の6.0.4.8更新作業を本仕様の完了範囲に含めない

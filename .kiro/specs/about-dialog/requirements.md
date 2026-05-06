# Requirements Document

## Introduction
MDLumaの"..."メニューにAbout項目を追加し、アプリのバージョン情報やビルド情報を確認できるAboutダイアログを提供する。

## Boundary Context
- **In scope**: "..."メニューへのAbout項目追加、Aboutダイアログの表示と内容、ビルド番号の生成、Sciter EULAで要求される帰属表示
- **Out of scope**: 自動アップデート機能、ライセンス情報ページ、サードパーティライブラリの一覧表示
- **Adjacent expectations**: "..."メニューはtheme-toggleやfont-settingsでも利用されるため、About項目はメニュー末尾に追加し既存項目の順序には影響しないこと

## Requirements

### Requirement 1: Aboutメニュー項目
**Objective:** MDLumaの利用者として、バージョン情報にアクセスできるよう、"..."メニューにAbout項目を表示したい。

#### Acceptance Criteria
1. When "..."ボタンをクリックしたとき、MDLuma shall ポップアップメニューの末尾に"About"項目を表示する。

### Requirement 2: Aboutダイアログの表示
**Objective:** MDLumaの利用者として、Aboutメニュー項目を押したときにバージョン情報を確認するダイアログを表示したい。

#### Acceptance Criteria
1. When ユーザーが"About"メニュー項目をクリックしたとき、 MDLuma shall Aboutダイアログをモーダルとして表示する。
2. While Aboutダイアログが表示されている間、 MDLuma shall ダイアログ背面の操作を無効にする。

### Requirement 3: Aboutダイアログの表示内容
**Objective:** MDLumaの利用者として、アプリの名前・バージョン・ビルド番号を一目で確認したい。

#### Acceptance Criteria
1. The Aboutダイアログ shall アプリのアイコンを表示する。
2. The Aboutダイアログ shall アプリ名"MDLuma"を表示する。
3. The Aboutダイアログ shall バージョン番号を表示する。
4. The Aboutダイアログ shall ビルド番号を表示する。

### Requirement 4: Sciter帰属表示
**Objective:** Sciter EngineのEULAに準拠するため、帰属テキストをAboutダイアログに表示したい。

#### Acceptance Criteria
1. The Aboutダイアログ shall "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc."というテキストを表示する。

### Requirement 5: ビルド番号の生成
**Objective:** 開発者として、ビルドごとに一意の識別子を付与し、どのコミット状態のビルドかを区別したい。

#### Acceptance Criteria
1. When ビルドを実行したとき、 MDLuma shall ビルド時点のgit commit hash短縮版をビルド番号として設定する。
2. If gitリポジトリが利用できない状態でビルドを実行したとき、 MDLuma shall ビルド番号として"unknown"を設定しビルドを失敗させない。

### Requirement 6: Aboutダイアログの操作
**Objective:** MDLumaの利用者として、Aboutダイアログを簡単に閉じたい。

#### Acceptance Criteria
1. When ユーザーがAboutダイアログのOKボタンをクリックしたとき、 MDLuma shall Aboutダイアログを閉じる。

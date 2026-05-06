# Requirements Document

## Introduction

MDLuma は、Markdown 本文の閲覧体験を利用者の読みやすさに合わせて調整できるよう、本文表示用のフォント種類とフォントサイズを変更する機能を提供する。利用者はタイトルバーの `...` メニューからフォント設定を開き、選択した設定を現在の本文表示へ反映できる。選択結果は次回起動時にも再利用される。

## Boundary Context (Optional)

- **In scope**: `...` メニューからのフォント設定導線、OS ネイティブのフォント設定ダイアログによる本文フォント種類とサイズの選択、現在表示中および以後に表示する Markdown 本文への適用、設定の保存と次回起動時の復元
- **Out of scope**: タイトルバー、メニュー、検索 UI、ウィンドウ操作ボタンなど本文以外のアプリケーション UI のフォント変更、テーマや配色の変更、OS が選択肢として提供しないフォントの追加
- **Adjacent expectations**: フォント設定の変更後も文書の閲覧、ファイル切り替え、本文テキスト選択は継続して利用できること。保存済み設定が利用できない場合でも既定の本文フォントで閲覧を継続できること

## Requirements

### Requirement 1: フォント設定へのアクセス

**Objective:** As a MDLuma ユーザー, I want タイトルバーから本文フォント設定を開けること, so that Markdown 本文の読みやすさを操作中に調整できる

#### Acceptance Criteria

1. When ユーザーがタイトルバーの `...` メニューを開いた, the MDLuma shall `Font` メニュー項目を表示する
2. When ユーザーが `Font` メニュー項目を選択した, the MDLuma shall OS ネイティブのフォント設定ダイアログを表示する
3. While フォント設定ダイアログが表示されている, the MDLuma shall Markdown 本文表示向けのフォント種類とフォントサイズを選択できるようにする
4. If ユーザーが選択を確定せずにフォント設定ダイアログを閉じた, then the MDLuma shall 現在の本文フォント設定を変更しない

### Requirement 2: Markdown 本文への適用範囲

**Objective:** As a MDLuma ユーザー, I want 選択したフォント設定が本文表示だけに反映されること, so that 閲覧性を調整しつつアプリケーション操作 UI の見た目は維持できる

#### Acceptance Criteria

1. When ユーザーがフォント設定ダイアログで選択を確定した, the MDLuma shall 現在表示中の Markdown 本文に選択したフォント種類とフォントサイズを適用する
2. While 本文フォント設定が有効である, the MDLuma shall タイトルバー、メニュー、検索 UI、およびその他のアプリケーション操作 UI のフォントを変更しない
3. When 別の Markdown 文書が表示された, the MDLuma shall 現在有効な本文フォント設定をその文書の Markdown 本文にも適用する
4. While 本文フォント設定が有効である, the MDLuma shall Markdown 本文のテキスト選択およびコピー操作を継続して利用可能にする

### Requirement 3: フォント設定の保存と起動時復元

**Objective:** As a MDLuma ユーザー, I want 選択した本文フォント設定が次回起動時にも復元されること, so that 毎回同じ設定をやり直さずに閲覧を始められる

#### Acceptance Criteria

1. When ユーザーが本文フォント設定を確定した, the MDLuma shall 選択したフォント種類とフォントサイズを利用者設定として保存する
2. When アプリケーションが起動し保存済みの本文フォント設定が利用可能である, the MDLuma shall 最初の Markdown 本文を表示する前にその設定を読み込む
3. When 保存済みの本文フォント設定が読み込まれた, the MDLuma shall セッション中に最初に表示する Markdown 本文へその設定を適用する
4. If 保存済みの本文フォント設定が存在しない, then the MDLuma shall 既定の本文フォント設定で Markdown 本文を表示する

### Requirement 4: 設定利用失敗時の継続動作

**Objective:** As a MDLuma ユーザー, I want フォント設定の保存や読込に失敗しても閲覧を継続できること, so that 設定の問題で Markdown を読めなくならない

#### Acceptance Criteria

1. If 保存済みの本文フォント設定を読み込めない, then the MDLuma shall 既定の本文フォント設定で起動を継続する
2. If 保存済みの本文フォント設定を現在の環境で適用できない, then the MDLuma shall 既定の本文フォント設定で Markdown 本文を表示する
3. If 本文フォント設定を保存できない, then the MDLuma shall 現在の閲覧セッションを継続し、障害内容を診断できる情報を残す

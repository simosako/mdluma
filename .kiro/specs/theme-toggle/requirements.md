# Requirements Document

## Introduction

MDLuma ユーザーが Markdown を light/dark いずれの環境でも快適に閲覧できるよう、UI 操作でテーマを切り替える機能を提供する。テーマ選択は設定ファイルに永続化され、次回起動時に自動的に復元される。設定ファイルは JSON 形式で OS の標準的なアプリケーションデータディレクトリに保存し、将来の設定項目追加にも対応できる構造とする。

## Boundary Context

- **In scope**: テーマ切り替えの UI 操作、切り替えに伴う表示変更、テーマ状態の永続化（設定ファイルへの読み書き）、起動時のテーマ復元
- **Out of scope**: カスタムテーマやテーマエディタ、OS の dark mode 設定との連動、テーマ以外の設定項目の追加（設定ファイルの構造のみ用意）
- **Adjacent expectations**: ドキュメント再読み込みやファイルオープン時に現在のテーマが維持されること

## Requirements

### Requirement 1: テーマ切り替え操作

**Objective:** As a MDLuma ユーザー, I want ツールバー上のボタンで light/dark テーマを切り替えられること, so that 現在の環境に合ったテーマで Markdown を閲覧できる

#### Acceptance Criteria

1. When ユーザーがテーマ切り替えボタンをクリックした, the MDLuma shall 現在のテーマを light から dark へ、または dark から light へ切り替える
2. While light テーマが有効な状態でテーマ切り替えボタンが表示されている, the MDLuma shall dark テーマへの切り替えを示すアイコンを表示する
3. While dark テーマが有効な状態でテーマ切り替えボタンが表示されている, the MDLuma shall light テーマへの切り替えを示すアイコンを表示する

### Requirement 2: テーマ切り替えに伴う表示変更

**Objective:** As a MDLuma ユーザー, I want テーマ切り替え時にすべての表示要素が一貫して切り替わること, so that 見苦しい色の不一致が生じない

#### Acceptance Criteria

1. When テーマが light から dark に切り替わった, the MDLuma shall 背景色、文字色、ボーダー色などのスタイルを dark 用の配色に変更する
2. When テーマが dark から light に切り替わった, the MDLuma shall 背景色、文字色、ボーダー色などのスタイルを light 用の配色に変更する
3. When テーマが切り替わった, the MDLuma shall すべてのツールバーアイコンを新しいテーマに対応するアイコンに変更する
4. While いずれかのテーマが適用されている, the MDLuma shall Markdown 本文を含むすべての表示領域で読みやすさを保つ配色を使用する

### Requirement 3: 起動時のテーマ復元

**Objective:** As a MDLuma ユーザー, I want 前回終了時のテーマ設定が起動時に自動的に復元されること, so that 毎回テーマを再設定する手間がない

#### Acceptance Criteria

1. When アプリケーションが起動した, the MDLuma shall 設定ファイルからテーマ設定を読み込み、そのテーマを適用する
2. When 設定ファイルが存在しない, the MDLuma shall light テーマを既定として適用する
3. When 設定ファイルは存在するがテーマ設定が含まれていない, the MDLuma shall light テーマを既定として適用する
4. When テーマ設定が読み込まれた, the MDLuma shall テーマ切り替えボタンを現在のテーマに対応するアイコンで有効状態で表示する

### Requirement 4: ドキュメント操作時のテーマ維持

**Objective:** As a MDLuma ユーザー, I want 別のファイルを開いたり検索を実行してもテーマが維持されること, so that 操作のたびにテーマを再設定する手間がない

#### Acceptance Criteria

1. When 新しい Markdown ファイルが開かれた, the MDLuma shall 現在のテーマを維持したまま新しいドキュメントを表示する
2. When 検索操作が実行された, the MDLuma shall 現在のテーマを維持したまま検索結果を表示する

### Requirement 5: 設定ファイルの保存

**Objective:** As a MDLuma ユーザー, I want アプリケーション終了時に現在の設定が自動的に保存されること, so that 次回起動時に設定が復元される

#### Acceptance Criteria

1. When アプリケーションが終了した, the MDLuma shall 現在のテーマ設定を設定ファイルに書き込む
2. When 設定ファイルの保存先ディレクトリが存在しない, the MDLuma shall ディレクトリを作成した上で設定ファイルを保存する
3. If 設定ファイルの書き込みに失敗した, then the MDLuma shall アプリケーションの終了を継続し、エラーを診断ログに出力する

### Requirement 6: 設定ファイルの形式と配置

**Objective:** As a MDLuma ユーザー, I want 設定が OS の標準的な場所に適切な形式で保存されること, so that 設定ファイルが見つけやすく、将来の設定項目追加にも対応できる

#### Acceptance Criteria

1. The MDLuma shall 設定ファイルを JSON 形式で保存する
2. The MDLuma shall 設定ファイルを OS のアプリケーションデータディレクトリに配置する
3. When 設定ファイルが存在するが内容が JSON として不正である, the MDLuma shall 既定値を使用して起動し、エラーを診断ログに出力する

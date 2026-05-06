# 要件定義書

## 概要

この機能は、MDLuma の外部エディタ起動先を利用者が変更可能にし、その設定を次回起動時にも維持できるようにすることを目的とする。MDLuma は軽量な読み取り専用ビューアーの境界を保ったまま、タイトルバーの `...` メニューから外部エディタ起動と設定変更の両方を提供する。

## 境界コンテキスト

- **対象範囲**: 利用者設定ファイルへの external editor 設定の保存と読込、`...` メニュー内 `External Editor` セクション配下への `External Editor Setting` 項目追加、当該項目選択時の OS 標準ファイル選択ダイアログ表示、利用者が選択した外部エディタ実行ファイルのフルパス保存、保存済み設定を用いた `External Editor` 起動先決定、未設定時の `notepad.exe` 利用
- **対象外**: MDLuma 自体の編集機能、外部エディタへの追加引数指定、OS 既定関連付けの解決、外部エディタ起動後の双方向同期、複数エディタの履歴管理、設定ファイルの手編集支援
- **隣接前提**: この機能は、既存の現在ファイル追跡と `External Editor` 起動導線が維持されることを前提とする。`External Editor Setting` は起動先の選択と保存のみを責務とし、外部エディタでの編集動作そのものは責務に含めない。

## 要件

### Requirement 1: `External Editor Setting` の操作導線

**Objective:** As a MDLuma 利用者, I want `...` メニューから外部エディタ設定を開けること, so that 設定ファイルを直接編集せずに起動先を変更できる

#### 受け入れ基準

1. When 利用者がタイトルバーの `...` メニューを開いた, the MDLuma shall `External Editor` の下に `External Editor Setting` 項目を表示する
2. When 利用者が `External Editor Setting` 項目を選択した, the MDLuma shall OS 標準のファイル選択ダイアログを表示する
3. While 利用者がファイル選択ダイアログを閉じるまで, the MDLuma shall 外部エディタ設定の変更を確定しない

### Requirement 2: 外部エディタ設定の保存と読込

**Objective:** As a MDLuma 利用者, I want 一度選んだ外部エディタを次回以降も使えること, so that 毎回同じ設定をやり直さずに済む

#### 受け入れ基準

1. When 利用者がファイル選択ダイアログで外部エディタ実行ファイルを選択した, the MDLuma shall そのフルパスを利用者設定ファイルへ保存する
2. Where 利用者設定ファイルに external editor 設定が保存されている, the MDLuma shall 起動時にその設定を読み込む
3. When 利用者が設定保存後に MDLuma を再起動した, the MDLuma shall 保存済みの external editor 設定を保持した状態で `External Editor` 起動に利用する
4. If 利用者がファイル選択ダイアログで選択を確定せずに終了した, then the MDLuma shall 既存の external editor 設定を変更しない

### Requirement 3: `External Editor` 起動先の決定

**Objective:** As a MDLuma 利用者, I want 設定済みの外部エディタまたは既定値で現在ファイルを開けること, so that 自分の編集環境へ一貫して移動できる

#### 受け入れ基準

1. Where 利用者設定ファイルに external editor 設定がある, the MDLuma shall `External Editor` 選択時にその設定先を起動対象として扱う
2. When 利用者設定ファイルに external editor 設定がない状態で利用者が `External Editor` を選択した, the MDLuma shall `notepad.exe` を起動対象として扱う
3. The MDLuma shall `External Editor` の 1 回の選択につき 1 つの実行ファイルのみを起動対象として扱う

### Requirement 4: 起動失敗時の継続利用

**Objective:** As a MDLuma 利用者, I want 外部エディタ設定や起動が失敗しても閲覧を続けられること, so that 設定操作の失敗で閲覧作業が中断されない

#### 受け入れ基準

1. If 設定された external editor の起動に失敗した, then the MDLuma shall 起動失敗を利用者に分かる形で表示する
2. If external editor の起動に失敗した, then the MDLuma shall 現在の MDLuma セッションを終了しない
3. If external editor 設定の保存に失敗した, then the MDLuma shall 設定失敗を利用者に分かる形で表示し閲覧を継続可能にする

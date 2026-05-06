# Requirements Document

## Introduction
MDLuma の現行 increment では、Windows 上でローカル Markdown ファイルを軽く素早く開き、読み取り専用の整形済み文書として表示できる最小ビューアー体験を提供する。この increment では、本文表示とは別に動いてしまう UI を避け、アプリケーション名、現在のファイル名、ビューアー操作、ウィンドウ操作を単一の統合タイトルバーにまとめた読み取り体験を提供する。requirements は、ユーザーから見える「起動する」「単一のタイトルバーで操作する」「本文をスクロールする」「Markdown を読む」「失敗時に状態を理解できる」範囲に限定し、編集、検索実行、テーマ切り替え実行、複数文書操作などの後続機能は含めない。

## Boundary Context
- **In scope**: デスクトップアプリケーションとしての起動、単一の統合タイトルバー、タイトルバー内のアプリ識別表示とファイル名表示、ファイルを開く操作、ウィンドウの最小化・最大化または元に戻す・終了、タイトルバーを固定したままの本文スクロール、単一のローカル Markdown ファイルの読み取り専用整形表示、読み込み失敗時のユーザー向けエラー、ローカル環境での閲覧
- **Out of scope**: Markdown 編集、保存、ファイル内検索の実行、テーマ切り替えの実行、複数タブ、複数文書同時表示、設定永続化、外部アプリケーション連携、macOS/Linux 対応、インストーラー作成
- **Adjacent expectations**: UI/theme、search、packaging、外部連携は後続 spec の候補であり、この spec では統合タイトルバーを備えた最小 Markdown 表示体験に必要な期待だけを扱う

## Requirements

### Requirement 1: 統合タイトルバーとウィンドウ基本操作
**Objective:** As a Windows ユーザー, I want ウィンドウ上端のタイトルと操作をひとつの場所で扱いたい, so that アプリの識別と基本操作を迷わず行える

#### Acceptance Criteria
1. When ユーザーが MDLuma のメインウィンドウを表示する, the MDLuma shall ウィンドウ最上部に単一の統合タイトルバーを表示する
2. The MDLuma shall 統合タイトルバー内にアプリケーションアイコンとアプリケーション名を表示する
3. If MDLuma のメインウィンドウが表示されている, then the MDLuma shall 統合タイトルバーとは別のユーザー可視なタイトル行またはメニュー行をその上下に追加表示しない
4. When ユーザーが統合タイトルバーの最小化操作を選択する, the MDLuma shall ウィンドウを最小化する
5. When ユーザーが統合タイトルバーの最大化または元に戻す操作を選択する, the MDLuma shall 対応するウィンドウ状態へ切り替える
6. When ユーザーが統合タイトルバーの閉じる操作を選択する, the MDLuma shall ウィンドウを閉じる
7. When ユーザーが統合タイトルバーのウィンドウ操作ボタン以外の領域をドラッグする, the MDLuma shall ウィンドウを移動できるようにする

### Requirement 2: 固定タイトルバーと本文スクロール
**Objective:** As a Markdown 読者, I want タイトルバーを見失わずに長い文書を読みたい, so that 読書中でも上部の情報と操作にすぐ戻れる

#### Acceptance Criteria
1. While ユーザーが MDLuma のウィンドウを表示している, the MDLuma shall 統合タイトルバーをウィンドウ上端に表示し続ける
2. When 表示中の文書内容が可視領域を超える, the MDLuma shall 文書表示領域のみをスクロール対象にする
3. The MDLuma shall 文書内容とエラー表示を統合タイトルバーの下に配置し、タイトルバーで隠さない

### Requirement 3: ローカル Markdown ファイルの読み込みとタイトル表示
**Objective:** As a Windows ユーザー, I want ローカル Markdown ファイルを開いて現在の対象を上部で確認したい, so that どの文書を読んでいるかをすぐ把握できる

#### Acceptance Criteria
1. While Markdown ファイルがまだ読み込まれていない, the MDLuma shall 統合タイトルバー内に Markdown ファイルを開くコントロールを表示する
2. When ユーザーが統合タイトルバーのファイルを開く操作を行う, the MDLuma shall ローカル Markdown ファイルを選択する流れを開始する
3. When Markdown ファイルが正常に読み込まれる, the MDLuma shall 読み込んだファイル名を統合タイトルバー内に表示する
4. If ユーザーがファイルを開く操作をキャンセルする, then the MDLuma shall 現在の表示状態を保持する
5. If 選択されたファイルを読み込めない, then the MDLuma shall アプリケーションを終了せずに読み込み失敗をウィンドウ内に表示する

### Requirement 4: Markdown 文書の整形表示
**Objective:** As a Markdown 読者, I want Markdown ソースを読み取りやすい文書として表示したい, so that ファイル内容を快適に確認できる

#### Acceptance Criteria
1. When Markdown ファイルが正常に読み込まれる, the MDLuma shall 統合タイトルバーの下に Markdown ソースではなく整形済み文書として内容を表示する
2. When Markdown ファイルに GitHub Flavored Markdown の構文が含まれる, the MDLuma shall 対応する構文を読み取り可能な整形表示として表示する
3. If Markdown ファイルに未対応または不正な構文が含まれる, then the MDLuma shall アプリケーションを終了せずに読み取り可能な範囲の内容を表示する
4. The MDLuma shall ローカル Markdown ファイルの表示にネットワーク接続を要求しない

### Requirement 5: ビューアー専用の読み取り体験
**Objective:** As a Markdown 読者, I want MDLuma が閲覧に集中したアプリケーションであってほしい, so that 誤って文書を変更する心配なく読める

#### Acceptance Criteria
1. The MDLuma shall Markdown ファイルを読み取り専用で表示する
2. The MDLuma shall この feature の範囲で Markdown 編集操作を提供しない
3. The MDLuma shall この feature の範囲で保存または上書き操作を提供しない
4. While ユーザーが Markdown ファイルを閲覧している, the MDLuma shall 閲覧操作の結果として元のファイル内容を変更しない

### Requirement 6: 統合タイトルバー内の最小コマンド範囲
**Objective:** As a プロジェクト関係者, I want 統合タイトルバーに含まれる現在機能と将来機能候補の境界が明確であってほしい, so that 最小 viewer の完了条件を誤解せずに評価できる

#### Acceptance Criteria
1. The MDLuma shall この feature の範囲を単一のローカル Markdown ファイルの表示に限定する
2. Where 統合タイトルバーにこの feature で未提供のコントロールが表示される, the MDLuma shall そのコントロールを利用不可であると分かる状態で表示する
3. If ユーザーがこの feature で利用不可のコントロールを選択する, then the MDLuma shall 現在の文書表示とウィンドウ状態を保持する
4. The MDLuma shall この feature の範囲でファイル内検索、テーマ切り替え、複数タブ、または複数文書同時表示を提供しない

### Requirement 7: 配布後の起動可能性
**Objective:** As a Windows ユーザー, I want 配布された MDLuma を開発環境なしで起動できる, so that 通常のデスクトップアプリケーションとして利用できる

#### Acceptance Criteria
1. When ユーザーが配布された MDLuma をサポート対象の Windows 環境で起動する, the MDLuma shall 開発環境の手動準備なしに起動する
2. The MDLuma shall ローカル Markdown 表示に必要な実行時ファイルを配布物に含める
3. If MDLuma が起動または表示に必要な条件を満たせない, then the MDLuma shall ユーザーまたは配布作業者が原因を確認できる失敗状態を示す

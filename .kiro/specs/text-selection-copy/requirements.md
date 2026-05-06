# Requirements Document

## Introduction
MDLuma の現行 increment では、読み取り専用の Markdown ビューアーとして表示中の文書内容をそのまま参照できるが、表示された本文上でテキストを選択してコピーすることはできない。この feature では、整形表示された Markdown 本文に対してマウス操作で文字列を選択し、一般的なコピー操作でクリップボードへ取り出せる閲覧体験を追加する。requirements は、本文表示領域における選択、選択済みテキストのコピー、失敗時の扱い、読み取り専用境界に限定し、編集や保存のようなビューアー外機能は含めない。

## Boundary Context
- **In scope**: 表示中の Markdown 本文に対するマウスドラッグ等でのテキスト選択、選択範囲の可視化、選択済みテキストのコピー、コピー不能時のユーザー可視な失敗扱い、文書切り替え時の選択状態更新
- **Out of scope**: Markdown 編集、保存、本文以外の UI 操作部のテキスト選択、画像や装飾要素そのもののコピー、複数箇所の同時選択、クリップボード履歴管理
- **Adjacent expectations**: クリップボード自体は OS が提供するが、この feature では少なくともコピー失敗がユーザーに分かり、成功時は閲覧状態を崩さずに継続利用できることを期待する

## Requirements

### Requirement 1: 本文テキストの選択
**Objective:** As a Markdown 読者, I want 表示中の本文から必要な箇所を選択したい, so that 必要な文言だけを取り出す準備ができる

#### Acceptance Criteria
1. When ユーザーが表示中の Markdown 本文上でテキスト選択を開始する, the MDLuma shall 本文表示領域内で選択範囲を作成できるようにする
2. While ユーザーが本文表示領域で選択操作を続けている, the MDLuma shall 複数行にまたがる連続した可視テキストを選択できるようにする
3. The MDLuma shall 選択された本文テキストをユーザーが判別できる見た目で表示する
4. If ユーザーが本文表示領域外または本文操作以外の UI 要素からドラッグを開始する, then the MDLuma shall その操作を本文テキスト選択として扱わない

### Requirement 2: 選択済みテキストのコピー
**Objective:** As a Markdown 読者, I want 選択した本文をそのままコピーしたい, so that 他のアプリケーションへ引用や共有ができる

#### Acceptance Criteria
1. When ユーザーが選択済みの本文テキストに対してコピー操作を行う, the MDLuma shall 選択中の文字列をシステムのクリップボードへコピーする
2. The MDLuma shall コピー結果に Markdown ソース記法ではなく表示中の本文テキストを含める
3. When 選択範囲に改行をまたぐ本文テキストが含まれる, the MDLuma shall 読み取れる順序を保った文字列としてコピーする
4. If ユーザーが選択範囲を持たない状態でコピー操作を行う, then the MDLuma shall 現在の表示状態を維持し、アプリケーションを終了しない

### Requirement 3: 選択状態の更新と読み取り専用境界
**Objective:** As a Markdown 読者, I want 選択状態が閲覧操作に自然に追従してほしい, so that ビューアーとして安心して操作できる

#### Acceptance Criteria
1. While 本文テキストが選択されている, the MDLuma shall 文書を読み取り専用のまま保持する
2. When ユーザーが別の本文箇所を選択し直す, the MDLuma shall 選択範囲を新しい選択内容へ更新する
3. When ユーザーが別の Markdown ファイルを開く, the MDLuma shall 以前の文書に対する選択状態を引き継がない
4. The MDLuma shall この feature の範囲で選択やコピーを通じて元の Markdown ファイル内容を変更しない

### Requirement 4: コピー失敗時の継続利用
**Objective:** As a Windows ユーザー, I want コピーできない場合でも原因を把握して閲覧を続けたい, so that 一時的な失敗で読書体験が壊れない

#### Acceptance Criteria
1. If MDLuma が選択済みテキストをコピーできない, then the MDLuma shall アプリケーションを終了せずにコピー失敗をユーザーが認識できる形で示す
2. If コピー失敗が発生する, then the MDLuma shall 表示中の文書内容とウィンドウ状態を保持する
3. The MDLuma shall この feature の範囲を本文表示領域のテキスト選択とコピーに限定し、編集または保存操作を追加しない

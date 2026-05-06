# Research & Design Decisions

## Summary
- **Feature**: `text-selection-copy`
- **Discovery Scope**: Extension
- **Key Findings**:
  - Sciter では通常要素でも `selectable` 属性を付けると `Element.Selection` を持てるため、Markdown 本文用 `article` を選択可能領域として扱える。
  - 選択文字列は `element.selection.toString()` で取得でき、クリップボード書き込みは `Clipboard.writeText()` で完結するため、この feature は Rust 側の新しい FFI を増やさず UI 資産内で閉じられる。
  - 現在の文書切り替えは `SciterLoadHtml` による全文再読み込みで実現されているため、文書変更時の選択状態リセットは既存の再描画境界に自然に乗せられる。

## Research Log

### Sciter の選択モデル
- **Context**: 本文テキストの選択をブラウザ前提で設計せず、Sciter の実際の契約に合わせる必要があった。
- **Sources Consulted**:
  - https://docs.sciter.com/docs/DOM/Element/
  - https://docs.sciter.com/docs/DOM/Element/Selection
- **Findings**:
  - `Element.Selection` は `<htmlarea>`、`<plaintext>`、および `selectable` 属性を持つ任意要素で利用できる。
  - 選択状態は `isCollapsed`、`type`、`toString()` などで判定できる。
  - `innerText` は「ユーザーが選択してコピーしたときに近い文字列」を返すが、現在選択中の範囲そのものは `selection.toString()` が直接表す。
- **Implications**:
  - `src/ui/index.html` が生成する `article.markdown-body` を選択境界の所有者にする。
  - 選択有無の判定とコピー対象文字列の抽出は `src/ui/app.js` に集約できる。

### Sciter のクリップボード API
- **Context**: コピー成功・失敗を OS クリップボードに対してどう判断するかを明確にする必要があった。
- **Sources Consulted**:
  - https://docs.sciter.com/docs/JS.runtime/
  - https://docs.sciter.com/docs/JS.runtime/Clipboard
  - https://docs.sciter.com/docs/DOM/Event
- **Findings**:
  - Sciter JS runtime は `Clipboard.writeText(string): boolean` を提供する。
  - キーボード操作は DOM `keydown` 系イベントで拾える。
  - 失敗は `false` 戻り値または例外で扱う設計にできる。
- **Implications**:
  - `Ctrl+C` / `Meta+C` を `src/ui/app.js` で検知し、選択文字列を `Clipboard.writeText()` に渡す。
  - クリップボード失敗は Rust の `ViewerState::ErrorVisible` へ混ぜず、UI 局所状態のメッセージとして表示する。

### 既存アーキテクチャへの統合
- **Context**: 既存の MDLuma は HTML シェル再生成で文書表示を差し替えるため、feature を最小変更で入れる必要があった。
- **Sources Consulted**:
  - `src/html_shell.rs`
  - `src/ui/index.html`
  - `src/ui/styles.css`
  - `src/ui/app.js`
  - `src/sciter/window.rs`
  - `design/sciter-custom-titlebar.md`
- **Findings**:
  - 本文領域は `html_shell.rs` が `<article class="markdown-body" ...>` として組み立てる。
  - UI 固有コマンドは `Window.this.xcall()` で Rust へ送っているが、本 feature は OS 連携を Sciter JS runtime だけで完結できる。
  - 現在の CSS は `.titlebar` に `user-select: none` を設定しているが、本文側に選択許可を明示していない。
- **Implications**:
  - Rust 側のコマンド列挙や FFI 境界は変更対象外にできる。
  - 本文選択とコピー失敗通知は `src/ui/index.html` / `styles.css` / `app.js` の 3 ファイルへ閉じ込める。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| UI-only selection and copy | Sciter DOM 選択と Clipboard API を `app.js` で直接扱う | 最小変更、FFI 追加不要、既存責務境界と整合 | 失敗通知を UI 局所状態で持つ必要がある | 採用 |
| Rust-mediated clipboard copy | JS から `xcall()` で選択文字列を Rust に渡し、Windows API でコピーする | OS 依存制御を Rust に集約できる | 新規コマンド、FFI、Windows 固有実装が増える | 現 scope では過剰 |
| 全文再構成による仮想選択 | Markdown ソースまたは DOM テキストを独自に再結合してコピー | DOM 非依存に見える | 実表示とコピー結果がずれやすい | 要件 2.2 に不利 |

## Design Decisions

### Decision: 本文 `article` を選択境界として所有する
- **Context**: Requirement 1 と 3 は本文だけを選択対象にし、タイトルバーや操作部は対象外にする必要がある。
- **Alternatives Considered**:
  1. `.content` 全体を選択可能にする
  2. `.markdown-body` だけを選択可能にする
- **Selected Approach**: `html_shell.rs` が出力する `article.markdown-body` に選択属性を付け、選択ロジックはその要素だけを見る。
- **Rationale**: 本文責務を単一点に閉じ込められ、既存の読み取り専用ビュー境界と一致する。
- **Trade-offs**: 本文外のエラーメッセージやタイトル名はこの feature ではコピー対象にならない。
- **Follow-up**: 複数行選択、空選択、再選択を UI テストで確認する。

### Decision: コピー失敗は ViewerState ではなく UI メッセージで扱う
- **Context**: Requirement 4 は失敗可視化を求めるが、既存の `ViewerState::ErrorVisible` は文書ロードやレンダリング失敗向けである。
- **Alternatives Considered**:
  1. Rust 側の `ViewerState` にコピーエラー状態を追加する
  2. UI 内の一時メッセージとして表示する
- **Selected Approach**: `app.js` がコピー失敗を捕捉し、`data-error-area` 配下の専用メッセージ領域を更新する。
- **Rationale**: 文書内容とウィンドウ状態を保持したまま失敗だけを通知でき、既存のロード系エラー責務を汚さない。
- **Trade-offs**: エラー種類は UI 文言に要約され、Rust の診断ログには現れない。
- **Follow-up**: 同一文書上でコピー失敗後も再選択と再コピーが継続できることを確認する。

### Decision: 一般的なコピー操作は `Ctrl+C` / `Meta+C` で扱う
- **Context**: 現行 UI に編集メニューやコンテキストメニューはなく、要件の「一般的なコピー操作」を最小構成で実装する必要がある。
- **Alternatives Considered**:
  1. キーボードショートカットのみ扱う
  2. Rust 側でメニュー統合まで広げる
- **Selected Approach**: `keydown` でコピーショートカットを処理し、選択があるときだけクリップボードへ書き込む。
- **Rationale**: Windows ビューアーの標準操作に合い、scope を viewer 本文に限定できる。
- **Trade-offs**: 将来メニューや右クリックコピーを追加する場合は再検証が必要になる。
- **Follow-up**: 後続 spec でメニューや検索を追加する場合は入力競合を見直す。

## Risks & Mitigations
- Sciter の `selectable` 属性だけでは視覚的選択が不足する可能性がある — `styles.css` で本文選択を妨げる指定を避け、必要なら選択色の明示を追加する。
- `Ctrl+C` 処理が他の将来ショートカットと衝突する可能性がある — `app.js` に本文コピー専用ハンドラをまとめ、条件分岐を局所化する。
- クリップボード失敗の再現が環境依存になりやすい — 失敗パスは `Clipboard.writeText` の戻り値 `false` と例外の両方をテスト対象にする。

## References
- [Sciter Element docs](https://docs.sciter.com/docs/DOM/Element/) — `innerText` と DOM 操作の基本契約
- [Sciter Element.Selection docs](https://docs.sciter.com/docs/DOM/Element/Selection) — 選択 API の仕様
- [Sciter Clipboard docs](https://docs.sciter.com/docs/JS.runtime/Clipboard) — システムクリップボード API
- [Sciter Event docs](https://docs.sciter.com/docs/DOM/Event) — `keydown` を含むイベント契約

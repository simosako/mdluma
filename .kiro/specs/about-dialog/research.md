# Research & Design Decisions Template

## Summary
- **Feature**: `about-dialog`
- **Discovery Scope**: Simple Addition
- **Key Findings**:
  - Rust→Sciterへのデータ渡しは`src/html_shell.rs`でのテンプレート置換（`{{PLACEHOLDER}}`）が唯一の仕組み
  - Sciterは`<dialog>`要素をサポートし、`showModal()`でモーダル表示可能
  - 既存のモーダル実装はなく、検索パネルはvisibility toggleパターンを使用

## Research Log

### Rust→Sciter データ渡しの仕組み
- **Context**: Aboutダイアログにバージョン・ビルド番号を表示するため
- **Sources**: `src/html_shell.rs:85-105`
- **Findings**: `{{PLACEHOLDER}}`形式のテンプレート置換を使用。Rust側で`str::replace()`によりHTMLテンプレート内のプレースホルダーを置換
- **Implications**: バージョン番号・ビルド番号も同様に`{{VERSION}}`・`{{BUILD_NUMBER}}`プレースホルダーで渡すのが最もシンプル

### Sciterのダイアログ機能
- **Context**: Aboutダイアログのモーダル表示方法を決定するため
- **Sources**: `vendor/sciter-js-sdk-main/docs/md/DOM/Window.md`
- **Findings**: Sciterは`<dialog>`HTML要素をサポート。`element.showModal()`でモーダル表示可能
- **Implications**: 検索パネルと同様のvisibility toggleではなく、Sciter標準の`<dialog>`要素を使用するのが適切

## Design Decisions

### Decision: バージョン情報の渡し方
- **Context**: Rustのバージョン番号・ビルド番号をSciter UIに表示する
- **Alternatives Considered**:
  1. テンプレート置換（`{{VERSION}}`） — 既存パターンに一致
  2. Rust→JSコール — 現在実装されていない仕組みが必要
- **Selected Approach**: テンプレート置換
- **Rationale**: 既存の`html_shell.rs`のパターンに沿う。新たなRust→JS通信仕組みは不要
- **Trade-offs**: HTMLリロードなしでは動的更新不可（Aboutダイアログでは問題なし）

### Decision: モーダルダイアログの実装
- **Context**: Aboutダイアログをモーダルとして表示する
- **Alternatives Considered**:
  1. `<dialog>`要素 + `showModal()` — Sciter標準
  2. `window.modal({params})` — 別Windowインスタンスとして表示
  3. div overlay + visibility toggle — 検索パネルと同じパターン
- **Selected Approach**: `<dialog>`要素
- **Rationale**: HTML5標準でSciterがサポート。バックドロップ付きモーダルが宣言的に書ける。別Window不要でシンプル
- **Trade-offs**: なし

### Decision: ビルド番号の生成
- **Context**: ビルドごとに一意の識別子を付与する
- **Alternatives Considered**:
  1. `git rev-parse --short HEAD` — コミットハッシュ短縮版
  2. タイムスタンプ — ビルド時刻
  3. 自動インクリメントカウンタ — ファイル保存
- **Selected Approach**: git commit hash短縮版
- **Rationale**: ユーザーの指定。コミット単位で一意・再現可能
- **Trade-offs**: gitリポジトリ外でのビルドでは"unknown"にフォールバック

## Risks & Mitigations
- gitが利用できない環境でのビルド — "unknown"をフォールバック値として使用しビルドを失敗させない

# Research & Design Decisions

## Summary
- **Feature**: `font-settings`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存の `SettingsFile -> AppController -> HtmlShell -> show_document()` の流れを拡張すれば、本文フォント変更・保存・再描画を最小追加で実現できる。
  - Sciter.js SDK には `button type="menu"` と `<menu.popup>` があり、`...` メニューを新しいポップアップ基盤なしで実装できる。
  - Windows `ChooseFontW` は `FALSE` だけでは失敗判定できず、`CommDlgExtendedError() == 0` をキャンセルとして扱う必要がある。

## Research Log

### 既存の設定保存と再描画経路
- **Context**: 要件 2 と 3 を既存アーキテクチャへどう載せるかを確認する必要があった。
- **Sources Consulted**:
  - `src/settings.rs`
  - `src/app.rs`
  - `src/html_shell.rs`
  - `src/lib.rs`
- **Findings**:
  - `Settings` は現状 `theme` のみを持つ JSON 設定モデルで、`#[serde(default)]` により欠落項目を補完できる。
  - `AppController` は起動時に設定を読み、設定変更時は HTML シェルを再生成して `show_document()` で差し替える。
  - 最初の文書表示前に設定読込を済ませる既存構造があり、起動時復元を追加しやすい。
- **Implications**:
  - 新規永続化層は不要であり、本文フォント設定は `Settings` 拡張で扱う。
  - フォント変更反映は部分 DOM 更新ではなく既存の全文再描画パターンを踏襲する。

### UI 導線と Sciter のメニュー手段
- **Context**: 要件 1.1 の `...` メニューと `Font` 項目を、Sciter の既存機能で最小構成実装できるかを確認した。
- **Sources Consulted**:
  - `src/ui/index.html`
  - `src/ui/app.js`
  - `vendor/sciter-js-sdk-main/docs/md/HTML/html-inputs.md`
  - `vendor/sciter-js-sdk-main/docs/md/behaviors/behavior-menu.md`
  - `vendor/sciter-js-sdk-main/docs/md/DOM/Element/README.md`
  - `vendor/sciter-js-sdk-main/docs/md/DOM/out-of-canvas-elements.md`
- **Findings**:
  - 現在の `more` ボタンは disabled で、メニュー導線は未実装である。
  - Sciter は `button type="menu"` に子 `<menu.popup>` を持たせるだけでクリック開閉メニューを提供する。
  - `data-action` と `Window.this.xcall(...)` の既存パターンをそのまま `font` 系操作へ拡張できる。
- **Implications**:
  - `...` メニューは HTML/JS 側の小変更で実装でき、Rust 側に UI 状態管理を追加する必要はない。
  - `Font` 項目は JS から `font-settings-requested` を送る薄いトリガに留める。

### Windows ネイティブフォントダイアログ契約
- **Context**: 要件 1.2, 1.3, 1.4 と 4 系の失敗時挙動を設計するため、Windows API の戻り値契約を確認した。
- **Sources Consulted**:
  - Microsoft Learn: `ChooseFontW`
  - Microsoft Learn: `CHOOSEFONTW`
  - Microsoft Learn: `LOGFONTW`
  - Microsoft Learn: `CommDlgExtendedError`
  - `src/platform/windows_file_dialog.rs`
- **Findings**:
  - `ChooseFontW` は `CHOOSEFONT` と `LOGFONT` を in/out で受け取り、選択結果を同じ構造体へ返す。
  - `ChooseFontW == FALSE` かつ `CommDlgExtendedError() == 0` はキャンセルであり、エラーではない。
  - フォント名は `lfFaceName`、サイズは `iPointSize`（1/10 pt 単位）で取得できる。
  - 既存の `WindowsFileDialog` は `Comdlg32` を直接叩く小さな FFI 境界であり、同じ責務配置を再利用できる。
- **Implications**:
  - `src/platform/windows_font_dialog.rs` を新設し、ファイルダイアログと同じく Windows 固有 FFI を局所化する。
  - 永続化モデルは `LOGFONT` 全体ではなく、要件に必要な本文フォント名とサイズだけを保持する。

### 本文限定適用とフォールバック
- **Context**: 要件 2.2 と 4.2 を満たしつつ、検索 UI やタイトルバーへ影響させない適用方式を決める必要があった。
- **Sources Consulted**:
  - `src/ui/styles.css`
  - `src/html_shell.rs`
  - `src/ui/index.html`
- **Findings**:
  - `html` ルートに既定フォントがあり、`search-input` は `font-family: inherit` を使っているため、ルート変更は UI 全体へ波及する。
  - Markdown 本文は `.markdown-body` と `.markdown-selection-host` に閉じ込められている。
  - CSS のフォントフォールバックチェーンを使えば、保存済みのフォント名が現環境で解決できない場合でも既定本文フォントへ自然に戻せる。
- **Implications**:
  - フォント設定は `.markdown-selection-host` または `.markdown-body` に限定した CSS 変数として注入する。
  - 起動時に完全な OS フォント列挙を追加せず、保存値の構文検証 + CSS フォールバックで 4.2 を満たす。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存ファイルへ全面集約 | `app.rs` と `windows_file_dialog.rs` へ寄せて実装 | 変更ファイル数が少ない | ダイアログ責務が混ざりやすい | 最小変更だが境界が曖昧になる |
| 新規境界を多めに分離 | 設定・ダイアログ・UI を広く分離 | 責務が明確 | この規模では過剰設計になりやすい | 軽量性方針とやや衝突 |
| ハイブリッド拡張 | 設定/再描画は既存拡張、ネイティブダイアログだけ新設 | 既存パターン再利用と責務分離の均衡が良い | 全文再描画による一時 UI 状態リセットは残る | **採用** |

## Design Decisions

### Decision: 既存 viewer 設定フローを拡張し、Windows ダイアログだけを新規境界に分離する
- **Context**: フォント設定は設定保存、現在表示の更新、起動時復元、Windows API 呼び出しの 4 領域をまたぐ。
- **Alternatives Considered**:
  1. 既存 `FileDialog` 周辺へ吸収する
  2. UI と設定も含めて複数の新規抽象へ分割する
- **Selected Approach**: `AppController`、`Settings`、`HtmlShell` は既存拡張とし、`FontDialog` / `WindowsFontDialog` だけを新しいプラットフォーム境界として追加する。
- **Rationale**: 既存の theme 実装に最も近く、Windows 固有責務だけを `src/platform/` に閉じ込められる。
- **Trade-offs**: アプリ全体の構造は保てる一方、全文再描画による一時 UI 状態リセットは維持される。
- **Follow-up**: `AppController` のジェネリクスとテスト初期化箇所が増えるため、テストユーティリティの整理を確認する。

### Decision: 永続化モデルは `BodyFontSettings` の最小値オブジェクトに限定する
- **Context**: Windows の `LOGFONT` は多くの属性を持つが、要件は本文フォント種類とサイズだけを要求する。
- **Alternatives Considered**:
  1. `LOGFONT` 相当を丸ごと保存する
  2. `family_name` と `point_size_tenths` だけを保存する
- **Selected Approach**: `Settings` に `Option<BodyFontSettings>` を追加し、`family_name` と `point_size_tenths` だけを永続化する。
- **Rationale**: 要件外の style/effects を持ち込まず、JSON 互換性と型安全性を保てる。
- **Trade-offs**: Windows ダイアログ上で見える style 選択結果は保存しない。
- **Follow-up**: フォント名の空文字と 0 サイズを無効値として扱う検証テストを追加する。

### Decision: `...` メニューは Sciter の `button type="menu"` を採用する
- **Context**: `Font` 項目の導線は必要だが、独自ポップアップ状態管理は増やしたくない。
- **Alternatives Considered**:
  1. 通常ボタン + 手動 popup 制御
  2. `button type="menu"` + 子 `<menu.popup>`
  3. `...` をやめて直接 Font ボタンを追加する
- **Selected Approach**: 既存の `more` ボタンを `button type="menu"` にし、子メニューに `Font` 項目を持たせる。
- **Rationale**: 要件 1.1 を最短で満たし、Sciter の既定メニュー挙動を使える。
- **Trade-offs**: メニュー内容は静的 DOM で持つため、動的項目追加には向かない。
- **Follow-up**: タイトルバーの drag 領域と menu click の干渉が起きないことを UI テストで確認する。

### Decision: 保存済みフォントの利用不能時は CSS フォールバックで既定本文フォントへ戻す
- **Context**: 要件 4.2 は、保存済み本文フォントが現在環境で適用できない場合も既定本文フォントで閲覧継続することを求める。
- **Alternatives Considered**:
  1. 起動時に Windows API で全フォント列挙して厳密検証する
  2. 保存値の構文だけ検証し、CSS フォントフォールバックチェーンで既定本文フォントへ戻す
- **Selected Approach**: 保存値は非空文字/非ゼロサイズだけを検証し、適用時は `selected, default, sans-serif` の順で CSS フォールバックを持たせる。
- **Rationale**: 新しい列挙 API やランタイム前提を増やさず、閲覧継続要件を満たせる。
- **Trade-offs**: 「適用不能だった」という事実を OS API で厳密検知はしない。
- **Follow-up**: `html_shell.rs` に CSS 文字列エスケープとフォールバック順序のテストを追加する。

### Decision: フォント変更反映は全文再描画を維持し、検索の一時状態保持は対象外とする
- **Context**: 既存のテーマ切替は全文再描画であり、検索 UI 状態は JS ローカル変数で保持される。
- **Alternatives Considered**:
  1. DOM の style だけ差し替えて検索状態を保持する
  2. テーマ切替と同じ全文再描画を使う
- **Selected Approach**: フォント変更も全文再描画とし、検索パネルやハイライトは必要時に再操作してもらう。
- **Rationale**: 既存設計との一貫性が高く、実装境界が最も小さい。
- **Trade-offs**: フォント変更時に検索クエリ、ハイライト、開閉状態は維持されない。
- **Follow-up**: design.md で非目標として明示し、検索機能自体が継続利用可能であることをテスト項目に落とす。

## Risks & Mitigations
- フォント変更時の全文再描画で検索の一時状態が消える — 非目標として明示し、再描画後も検索 UI を再度開いて利用できることを確認する。
- フォント名を CSS 文字列へ埋め込む際に壊れた文字列を生成する可能性がある — `HtmlShell` に専用の CSS エスケープと単体テストを追加する。
- `ChooseFontW` の所有ウィンドウを渡さないため、表示位置やフォーカスが完全最適ではない可能性がある — 既存 file dialog と同じ制約として扱い、必要になった時点で別 spec で owner handle 連携を検討する。

## References
- `src/settings.rs` — 既存設定保存とフォールバック方針
- `src/app.rs` — 設定変更時の再描画制御
- `src/html_shell.rs` — 本文描画境界と shell 組み立て
- `src/ui/index.html` — タイトルバー導線の現状
- `src/ui/app.js` — `Window.this.xcall(...)` ブリッジの既存パターン
- `src/ui/styles.css` — ルートフォントと本文スタイルの現状
- `src/platform/windows_file_dialog.rs` — Windows 固有 FFI 実装パターン
- `vendor/sciter-js-sdk-main/docs/md/HTML/html-inputs.md` — `button type="menu"` の仕様
- `vendor/sciter-js-sdk-main/docs/md/behaviors/behavior-menu.md` — Sciter popup menu の仕様
- `vendor/sciter-js-sdk-main/docs/md/DOM/out-of-canvas-elements.md` — popup lifecycle と out-of-canvas popup
- [ChooseFontW](https://learn.microsoft.com/windows/win32/api/commdlg/nf-commdlg-choosefontw) — Windows ネイティブフォントダイアログ
- [CHOOSEFONTW](https://learn.microsoft.com/windows/win32/api/commdlg/ns-commdlg-choosefontw) — ダイアログ入出力構造体
- [LOGFONTW](https://learn.microsoft.com/windows/win32/api/wingdi/ns-wingdi-logfontw) — フォント名とサイズ表現
- [CommDlgExtendedError](https://learn.microsoft.com/windows/win32/api/commdlg/nf-commdlg-commdlgextendederror) — キャンセルと実エラーの判別

# Implementation Plan

- [ ] 1. テーマ状態の定義とコマンドルーティング
- [x] 1.1 Theme 列挙体と IconName バリアントの定義
  - light/dark を表す Copy 列挙体 Theme を定義する（Default は Light）
  - toggle(), theme_attr(), icon_theme(), toggle_icon() メソッドを実装する
  - IconName に Sun, Moon バリアントを追加する（toggle_icon() が型を返すために必要）
  - 完了条件: cargo test で Theme の全メソッドと IconName バリアントが正しく動作する
  - _Requirements: 1.1, 3.1_

- [x] 1.2 ViewerCommand へのテーマ切替コマンド追加
  - ThemeToggleRequested バリアントを追加する
  - parse_scripting_method_call で "theme-toggle-requested" をマッピングする
  - from_element_action で "theme" をマッピングする
  - 完了条件: cargo test で文字列からのパースが正しく ThemeToggleRequested を返す
  - _Requirements: 1.1_
  - _Boundary: Sciter Bridge_

- [ ] 2. CSS テーマ変数の定義と色値の移行
- [x] 2.1 (P) ハードコード色値を CSS 変数に抽出する
  - html:not(:theme(dark)) セレクタで light パレットを CSS 変数として定義する
  - html:theme(dark) セレクタで dark パレットを CSS 変数として定義する
  - 既存のハードコード色値を var(--viewer-*) 参照に置換する
  - .theme-dot クラスを削除する
  - 完了条件: すべての色値が CSS 変数参照になり、ハードコード色が残っていない
  - _Requirements: 2.1, 2.2, 2.4_
  - _Boundary: Presentation_

- [ ] 3. UI 側テーマ切り替え操作の実装
- [x] 3.1 (P) テーマ切り替えボタンの有効化とテンプレート対応
  - index.html のテーマボタンから disabled 属性を削除する
  - テーマアイコンを img 要素 + {{THEME_ICON}} プレースホルダーに変更する
  - html 要素の theme 属性を {{THEME_ATTR}} プレースホルダーに変更する
  - title / aria-label を更新する
  - 完了条件: テーマボタンが有効状態で表示され、プレースホルダーが埋め込み可能な形になっている
  - _Requirements: 1.2, 1.3, 3.2_
  - _Boundary: HTML Shell_

- [x] 3.2 テーマ切替の xcall 送信ロジックを app.js に追加する
  - handleClick で data-action="theme" を処理する分岐を追加する
  - requestThemeToggle 関数を実装し xcall theme-toggle-requested を送信する
  - 完了条件: テーマボタンクリック時に xcall が Rust 側に送信される
  - _Requirements: 1.1_
  - _Boundary: Sciter Runtime_
  - _Depends: 3.1_

- [ ] 4. Rust 側テーマ統合
- [x] 4.1 ShellModel へのテーマ情報統合
  - ShellModel に theme フィールドを追加する
  - render_shell で {{THEME_ATTR}} を theme.theme_attr() で置換する
  - render_shell で {{THEME_ICON}} を theme.toggle_icon() のアイコン URL で置換する
  - すべてのツールバーアイコンを theme.icon_theme() で解決するよう変更する
  - 完了条件: Theme::Dark を渡したとき、生成 HTML に theme="dark" が含まれ、アイコンが Dark テーマで解決される
  - _Requirements: 1.2, 1.3, 2.3, 4.1, 4.2_
  - _Boundary: HTML Shell_
  - _Depends: 1.1_

- [x] 4.2 AppController へのテーマ状態追加とハンドラ実装
  - AppController に theme: Theme フィールドを追加し Theme::default() で初期化する
  - ThemeToggleRequested のハンドラで theme.toggle() を呼び出しシェルを再描画する
  - render_state_html で ShellModel に theme を渡す
  - テスト用 with_theme() メソッドを追加する
  - 完了条件: テーマ切替ハンドラが呼ばれると theme がトグルされ、シェル再描画が発生する
  - _Requirements: 1.1, 3.1, 4.1, 4.2_
  - _Boundary: Application Control_
  - _Depends: 1.1, 1.2, 4.1_

- [ ] 5. テーマ切替の検証
- [x] 5.1 テーマ切替の統合テスト
  - AppController でテーマ切替後に ShellModel 出力が正しく更新されることを検証する
  - ファイルオープン後にテーマが維持されることを検証する
  - 完了条件: cargo test で統合テストがパスする
  - _Requirements: 1.1, 4.1, 4.2_
  - _Depends: 4.2_

- [x] 5.2* テーマ切替の Node.js UI テスト
  - handleClick が data-action="theme" のクリックで xcall を送信することを検証する
  - 無効化されたテーマボタンは無視されることを検証する
  - 完了条件: Node.js テストハーネスでテーマ関連 UI テストがパスする
  - _Requirements: 1.1, 3.2_
  - _Depends: 3.2_

- [ ] 6. 設定ファイル永続化の基盤構築
- [x] 6.1 serde/serde_json 依存の追加と設定モジュールの登録
  - Cargo.toml に serde（derive feature付き）と serde_json を dependencies に追加する
  - src/lib.rs に mod settings; を追加する
  - src/settings.rs ファイルを作成する
  - 完了条件: cargo check がパスし、settings モジュールが crate からアクセス可能になる
  - _Requirements: 6.1_

- [x] 6.2 ThemePreference 列挙体と Settings 構造体の実装
  - ThemePreference 列挙体（Light, Dark）に serde 属性と From<Theme>/Into<Theme> 変換を実装する
  - Settings 構造体に theme フィールドと #[serde(default)] を追加し Default を実装する
  - 完了条件: cargo test で ThemePreference の serde シリアライズ/デシリアライズと双方向 Theme 変換が正しく動作する
  - _Requirements: 3.1, 3.3, 5.1, 6.1_
  - _Boundary: Application Config_
  - _Depends: 6.1_

- [x] 6.3 SettingsFile の読み込み・書き込み機能の実装
  - %LOCALAPPDATA%\MDLuma\settings.json のパス解決とテスト用 with_path() を実装する
  - load() でファイル不在・読み込み失敗・JSON 不正時に Settings::default() を返す
  - save() で親ディレクトリ作成（create_dir_all）と JSON 書き込み（to_string_pretty）を実装する
  - すべてのエラーを debug_log! で出力し呼び出し元に伝播させない
  - LOCALAPPDATA 環境変数未設定時はテンポラリディレクトリにフォールバックする
  - 完了条件: cargo test でファイル不存在時・不正 JSON 時の load() が default を返し、save() が正しい JSON を書き込む
  - _Requirements: 3.1, 3.2, 5.1, 5.2, 5.3, 6.2, 6.3_
  - _Boundary: Application Config_
  - _Depends: 6.2_

- [ ] 7. AppController への設定永続化統合
- [x] 7.1 起動時のテーマ復元と切替時の設定保存
  - AppController に SettingsFile フィールドを追加する
  - new()/with_launcher() で SettingsFile::load() からテーマを読み込み初期化する
  - toggle_theme() でテーマ切替後に SettingsFile::save() を呼び出す
  - テスト用 with_settings_file() メソッドを追加する
  - 完了条件: cargo test で設定ファイルから dark テーマが読み込まれ、テーマ切替後に設定が保存される
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.1, 4.2, 5.1_
  - _Boundary: Application Control_
  - _Depends: 6.3_

- [ ] 8. 設定永続化の検証
- [x] 8.1* AppController 設定統合の統合テスト
  - 起動時に設定ファイルから dark テーマが読み込まれ適用されることを検証する
  - 設定ファイル不存在時に light テーマが適用されることを検証する
  - テーマ切替後に設定ファイルが更新されることを検証する
  - 連続切替後に正しいテーマが保存されることを検証する
  - 完了条件: cargo test で全統合テストがパスする
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 5.1_
  - _Depends: 7.1_

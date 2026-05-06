# Implementation Plan

- [x] 1. Build number generation
  - build.rsのmain()内で`git rev-parse --short HEAD`を実行し、結果を`cargo:rustc-env=GIT_COMMIT_HASH=<hash>`として設定する
  - gitコマンドが利用できない場合は`cargo:rustc-env=GIT_COMMIT_HASH=unknown`を設定しpanicしない
  - Observable: gitリポジトリ内で`cargo build`後に`env!("GIT_COMMIT_HASH")`が7桁程度のハッシュ文字列を返し、リポジトリ外では"unknown"を返す
  - _Requirements: 5.1, 5.2_
  - _Boundary: Build Number Generator_

- [x] 2. Version info template substitution
  - html_shell.rsのテンプレート置換チェーンに`{{VERSION}}`を`env!("CARGO_PKG_VERSION")`の値で置換する処理を追加する
  - 同チェーンに`{{BUILD_NUMBER}}`を`env!("GIT_COMMIT_HASH")`の値で置換する処理を追加する
  - Observable: 生成されたHTML内で`{{VERSION}}`が`CARGO_PKG_VERSION`（Cargo.tomlのversionフィールド値）に、`{{BUILD_NUMBER}}`がgitハッシュ（または"unknown"）に置換される
  - _Depends: 1_
  - _Requirements: 3.3, 3.4_
  - _Boundary: Version Template Substitution_

- [x] 3. About menu item and dialog UI
- [x] 3.1 Add About menu item and dialog HTML/CSS
  - "..."メニューの`<menu.popup>`内末尾に`<li data-action="about">About</li>`を追加する
  - `<dialog>`要素でAboutダイアログを構築する：アプリアイコン（既存の`{{APP_ICON}}`を使用）、アプリ名"MDLuma"、バージョン（`{{VERSION}}`）、ビルド番号（`{{BUILD_NUMBER}}`）、Sciter帰属テキスト、OKボタンを配置する
  - ダイアログを画面中央に表示するスタイルを追加する
  - Observable: "..."メニューを開くと末尾に"About"項目が表示される; HTMLにdialog要素が存在する
  - _Depends: 2_
  - _Requirements: 1.1, 3.1, 3.2, 3.3, 3.4, 4.1_
  - _Boundary: About Dialog_

- [x] 3.2 Implement About dialog show/close logic
  - handleClick()に`"about"`アクションの分岐を追加し、`document.querySelector('dialog')`に対して`showModal()`を呼び出す
  - OKボタンのクリックイベントで`dialog.close()`を呼び出す
  - Observable: "About"をクリックすると中央にモーダルダイアログが表示され、背面操作が無効になる; OKボタンでダイアログが閉じる
  - _Depends: 3.1_
  - _Requirements: 2.1, 2.2, 6.1_
  - _Boundary: About Dialog_

- [x] 4. Integration verification
  - gitリポジトリ内・外の双方でビルドが成功し、ビルド番号が正しく設定されることを確認する
  - Aboutダイアログを開き、アイコン・アプリ名・バージョン・ビルド番号・帰属テキストが正しく表示されることを確認する
  - OKボタンでダイアログが閉じることを確認する
  - Observable: 全ての受入基準を満たすことが手動テストで確認できる
  - _Depends: 2, 3.2_
  - _Requirements: 1.1, 2.1, 2.2, 3.1, 3.2, 3.3, 3.4, 4.1, 5.1, 5.2, 6.1_
  - _Boundary: Integration_

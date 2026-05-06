# Brief: about-dialog

## Problem
アプリのバージョン情報やビルド情報をユーザーが確認する手段がない。

## Current State
"..."メニューにFontとExternal Editorの項目があるが、About項目はない。
バージョン情報はCargo.tomlに`0.1.0`として定義されているが、ビルド番号の仕組みはない。
モーダルダイアログの実装も存在しない。

## Desired Outcome
"..."メニューのAbout項目を押すと、アプリのアイコン・名前・バージョン・ビルド番号・Sciter帰属表示・OKボタンを含むAboutダイアログ（モーダル）が表示される。

## Approach
build.rsで`git rev-parse --short HEAD`を実行しビルド番号を`rustc-env`に設定。RustからSciterへバージョン・ビルド番号を渡し、Sciterの`<dialog>`要素でAboutモーダルを実装する。

## Scope
- **In**: "..."メニューへのAbout項目追加、AboutダイアログのUI実装、ビルド番号生成（git commit hash短縮版）、Sciter EULAで要求される帰属表示
- **Out**: バージョンの自動更新確認、リリースノート表示、アップデート機能

## Boundary Candidates
- ビルド番号の生成（build.rs）
- バージョン情報のRust→Sciter連携
- AboutダイアログのUI（HTML/CSS/JS）

## Out of Boundary
- 自動アップデート機能
- ライセンス情報ページ
- サードパーティライブラリの一覧表示

## Upstream / Downstream
- **Upstream**: Cargo.tomlのバージョン番号、gitリポジトリ（ビルド番号取得）
- **Downstream**: なし

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: theme-toggle, font-settings（同じ"..."メニューを共有）

## Constraints
- Sciter EULAで要求される帰属テキスト: "This application uses Sciter Engine (http://sciter.com/), copyright Terra Informatica Software, Inc."
- ビルド番号はgit commit hash短縮版を使用
- モーダルダイアログとしてOKボタンで閉じる

# initial design 
MDLumaは、Markdownファイルを表示するビューアーアプリケーション

# デザイン
- 軽量、高速起動、省メモリ設計を重視する
- デザインはシンプルかつミニマル
- ファイル編集機能といったビューアーに関係ない機能は実装しない。ただし外部アプリケーションの連携等は追加しても良い
- Windows 11/10 で動作するアプリケーション(*.exe)として実装する。将来的にはMac OSやLinuxでも動作するよう拡張するので、ポータビリティを意識した設計
- サポートするMarkdownは GitHub Flavored Markdown (GFM) の範囲とするが、実装が困難なものは非サポートにする可能性がある https://github.github.com/gfm/

# 利用技術・コンポーネント
アプリケーションはMarkdownファイルを読み込み、それをまずHTMLに変換したうえで、HTMLとしてレンダリングする構成。

## 開発言語
- Rust : ビルドターゲット x86_64-pc-windows-msvc をデフォルトとする

## 利用ライブラリ：レンダリング
HTMLを受け取り、画面に描画する。（必要であれば）メニューバーの描画等も担当する。
- Sciter.js SDK https://sciter.com/download/ (古いSciter.TISと混同しないよう注意)

Rust から Sciter.js SDK を利用する際は、古い Sciter.TIS 世代の Rust ラッパーに依存せず、Sciter.js SDK の C API を小さなローカル FFI 境界で呼び出す方針とする。

配布時は単一の静的リンク exe に固執せず、必要な Sciter DLL をアプリケーションに同梱する方針とする。

## 利用ライブラリ： MarkdownからHTMLへの変換
Markdownを受けとり、HTMLへ変換する。

- Comrak https://github.com/kivikakk/comrak , https://docs.rs/comrak/latest/comrak/index.html

comrak::markdown_to_html を使うことでhtmlを得ることができる。

**注意** 今回はCommon Markの範囲に加え、GFMをサポートする必要があるので、comrakでもgfmを有効にします。

### 補足：markdownライブラリの代替案
もしComrakで機能が不足する等の問題がある場合は以下を検討する
https://github.com/pulldown-cmark/pulldown-cmark
https://github.com/wooorm/markdown-rs

## 補足：Visual Sudio Build Tools 2022 のツール(nmake.exe等)を呼び出す必要がある場合
もしnmake.exe等にPATHが通っていない場合は、(PowerShell用に定義された) Enter-Vs 関数を実行することで各種パスが設定される。

# UI
UIはダークモードとライトモードを選択可能にする。
UIのサンプルイメージは``design/initial-image.png``置いているので、GUIを設計する際は参照すること（画像左がライトモード、右がダークモード）

## メニュー
メニューはタイトルバーに置く形式である。
前述のサンプルイメージにあるように、メニューは左から、

アプリを表すアイコン(SVG), アプリ名, 読み込んだMarkdownファイル名, ファイルオープンアイコン, ファイル内検索アイコン, テーマ切り替えアイコン, "…"(その他のメニューを実装したとき用)である。

これをHTMLで実装したサンプルが ``design/sample-index.html`` にあるので、必要に応じてこれを参照する。

## アイコン
UIに利用するアイコン(.svg)と、アプリに登録するアイコン(.ico)は作成済であり、 assets/ 以下に保存されているので、これを利用します。

``assets/app.ico`` - アプリケーションアイコン
``assets/light/*.svg`` ライトテーマ時の各種アイコン (svg)
``assets/dark/*.svg`` ダークテーマ時の各種アイコン (svg)

# 開発手法
一度にアプリケーション要件を全部作る必要はありません。段階的に機能を追加していく開発手法・スタイルです。

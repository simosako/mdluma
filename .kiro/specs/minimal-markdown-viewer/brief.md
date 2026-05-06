# Brief: minimal-markdown-viewer

## Problem
MDLuma の最初の利用者は、Windows 上で Markdown ファイルを軽く素早く開いて読むための専用ビューアーを必要としている。既存状態ではアプリ本体がまだなく、`design/initialdesign.md` にある「Markdown を HTML に変換して表示する」最小体験を確認できない。

## Current State
リポジトリには設計資料と UI/アイコン素材が存在するが、Rust アプリケーション、Markdown 読み込み、HTML 変換、Sciter 表示の実装はまだ存在しない。既存 spec も steering 文書も未作成のため、この作業が最初のアプリケーション spec になる。

## Desired Outcome
Windows 11/10 向けの軽量な MDLuma 実行ファイルとして起動し、ローカル Markdown ファイルを読み込み、Comrak の GFM 対応で HTML に変換し、Sciter.js SDK ベースの最小 UI で表示できる。初回実装は「表示できること」を優先し、後続の UI 強化や検索機能を追加しやすい境界を残す。

## Approach
縦切り MVP として、Rust でファイル読み込みと Markdown 変換を実装し、変換済み HTML を Sciter.js SDK のウィンドウへ渡して表示する。`sciter-rs` のような古い Sciter/TIS 世代の Rust ラッパーには依存せず、Sciter.js SDK の C API に対する小さなローカル FFI 境界を用意する。配布は静的単体 exe に固執せず、必要な Sciter DLL 同梱を許容する。

## Scope
- **In**: Rust アプリケーションの最小構成、Markdown ファイル読み込み、Comrak による GFM 対応 HTML 変換、Sciter.js SDK による HTML 表示、最小限のタイトルバー/ビューアー UI、`assets/` の既存アイコン利用方針、Windows x86_64 MSVC ターゲットでのビルド確認
- **Out**: Markdown 編集機能、ファイル内検索、その他メニューの詳細機能、外部アプリ連携、複数タブ、設定永続化、完全な GFM 互換性保証、macOS/Linux 対応、単一静的 exe 配布

## Boundary Candidates
- Markdown 入力と HTML 変換: ファイルパスから Markdown を読み込み、Comrak 設定を通して HTML 文字列を生成する責務
- HTML 表示と UI シェル: Sciter.js SDK へ HTML を渡し、最小ウィンドウと viewer UI を表示する責務
- プラットフォーム/配布境界: Windows MSVC ビルド、Sciter DLL 配置、アプリケーションアイコンの扱いを分離する責務

## Out of Boundary
- ビューアー以外の編集機能は持たない
- 検索、テーマ切り替え、その他メニューは初回 MVP の必須完了条件にしない
- Sciter の静的リンクや有償ライセンス前提の配布方式はこの spec で扱わない
- 将来の macOS/Linux 対応は設計上の妨げを作らない範囲に留め、実装対象にはしない

## Upstream / Downstream
- **Upstream**: `design/initialdesign.md`、`design/index.html`、`design/initial-image.png`、`assets/` 配下の既存アイコン、Rust、Comrak、Sciter.js SDK
- **Downstream**: テーマ切り替え、ファイル内検索、最近使ったファイル、外部アプリ連携、macOS/Linux 対応、配布パッケージング

## Existing Spec Touchpoints
- **Extends**: なし
- **Adjacent**: 後続の UI/theme spec、search spec、packaging spec と境界が重なる可能性があるため、今回は最小 viewer 表示に限定する

## Constraints
軽量、高速起動、省メモリ、シンプルでミニマルな viewer 専用アプリとして実装する。デフォルトの Rust ビルドターゲットは `x86_64-pc-windows-msvc` とする。Markdown は HTML に変換してから表示し、変換には Comrak を使って GFM を有効にする。HTML レンダリングには Sciter.js SDK を使い、古い Sciter.TIS や古い Rust ラッパーと混同しない。Sciter の静的リンクやソースアクセスは無償前提にしない。

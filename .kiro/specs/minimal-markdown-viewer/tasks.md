# Implementation Plan

- [x] 1. 基盤: 起動前提と単一ウィンドウホストの土台を整える
- [x] 1.1 Sciter 実行時前提と配布時の runtime 依存検証を明確にする
  - 実行ファイル近傍の Sciter DLL を起動前に検証し、足りない場合は user/operator の双方が原因を追える失敗状態を返す。
  - 開発環境に依存しない起動条件がコードと build 出力の両方で確認できるようにする。
  - Sciter DLL 欠落時でも何が不足しているかを確認できる診断が表示または出力される状態で完了とする。
  - _Requirements: 7.1, 7.2, 7.3_
  - _Boundary: SciterRuntime_

- [x] 1.2 二重 chrome を避ける viewer window 作成モードを確定する
  - OS 既定の追加タイトル行を見せず、統合タイトルバーを唯一の上部 UI にできる window 作成設定へ更新する。
  - 起動直後の empty shell が単一ウィンドウ・単一文書前提で表示され、複数文書や追加メニュー行の入口を持たない構成にする。
  - 起動時にユーザー可視な上部バーが 1 本だけになる前提がコード上で確認できる状態で完了とする。
  - _Verification: 2026-04-29 Windows 実機で native の Windows title bar が消え、上部バーが二重表示にならないことを確認した。_
  - _Requirements: 1.1, 1.3, 6.1, 6.4, 7.1_
  - _Boundary: SciterWindow_

- [x] 2. Core: 統合タイトルバー shell とネイティブ操作を実装する
- [x] 2.1 (P) Windows 固有の最小 window chrome adapter を実装する
  - HWND に対する minimize、maximize/restore、close、begin drag だけを platform 境界に閉じ込める。
  - viewer state や document pipeline に触れず、OS window state だけを変更する。
  - 各 command が成功すると対応する window state change が起きる状態で完了とする。
  - _Requirements: 1.4, 1.5, 1.6, 1.7_
  - _Boundary: WindowsWindowChrome_

- [x] 2.2 (P) 統合タイトルバーで使うローカル icon と shell asset の解決を整える
  - app icon と window control icon をローカル resource として解決し、shell へ安全に注入できるようにする。
  - titlebar が使う style、script、icon に remote URL を混ぜない resource policy を明示する。
  - 統合タイトルバーに必要な asset が local-only で取得できる状態で完了とする。
  - _Requirements: 1.2, 4.4, 6.2_
  - _Boundary: UiAssets_

- [x] 2.3 統合タイトルバー shell にアプリ識別、ファイル名、利用不可コントロールを載せる
  - アプリアイコン、アプリ名、現在ファイル名の表示枠、open control、将来機能用 control、window control の並びを統合タイトルバーにまとめる。
  - search / theme / more は利用不可と分かる表示に固定し、編集や保存の affordance を出さない。
  - 文書未読込時でも open control が見え、文書読込後は同じ titlebar 内でファイル名が確認できる状態で完了とする。
  - _Depends: 2.2_
  - _Requirements: 1.1, 1.2, 3.1, 3.3, 5.2, 5.3, 6.2, 6.4_
  - _Boundary: DefaultHtmlShell_

- [x] 2.4 固定タイトルバーと本文専用スクロールのレイアウトを成立させる
  - タイトルバーと本文/エラー表示を sibling 構造にし、本文表示領域だけがスクロールするようにする。
  - titlebar の高さぶんだけ本文や error が下に配置され、上端 UI に隠れないようにする。
  - 長文を表示しても titlebar が上端に残り、本文だけが動く状態で完了とする。
  - _Depends: 2.3_
  - _Requirements: 2.1, 2.2, 2.3, 4.1_
  - _Boundary: DefaultHtmlShell_

- [x] 2.5 タイトルバー入力を open-file と window chrome command に変換する
  - open、minimize、maximize/restore、close、drag の入力だけを custom event に変換する。
  - disabled control への入力は握りつぶし、文書表示と window state を変えない。
  - ボタン以外の drag region からだけ window-drag request が出る状態で完了とする。
  - _Depends: 2.3_
  - _Requirements: 1.4, 1.5, 1.6, 1.7, 3.2, 6.2, 6.3_
  - _Boundary: TitleBarInteractionScript_

- [x] 3. Integration: titlebar shell と既存 viewer pipeline を接続する
- [x] 3.1 統合タイトルバーの command routing を Sciter window に組み込む
  - open-file-requested だけを app controller へ渡し、window-* は native adapter 側で完結させる。
  - raw HWND や OS 固有 state を app layer へ漏らさず、window command が document state を変えないようにする。
  - titlebar の open と window controls が同じ shell 上で動作しても責務分離が保たれる状態で完了とする。
  - _Depends: 1.2, 2.1, 2.5_
  - _Requirements: 1.4, 1.5, 1.6, 1.7, 3.2, 6.3, 7.1_
  - _Boundary: SciterWindow_

- [x] 3.2 統合 shell で整形済み Markdown 表示と local-only resource policy を成立させる
  - GFM 対応の整形済み HTML fragment を titlebar 下の本文領域へ収め、Markdown ソースをそのまま見せない。
  - 未対応または不正寄りの構文でも読める範囲を表示し、shell に埋め込む style、script、icon は remote resource を参照しないようにする。
  - GFM 文書が titlebar 下に読みやすく表示され、network access なしで viewer が成立する状態で完了とする。
  - _Depends: 2.2, 2.3, 2.4_
  - _Requirements: 4.1, 4.2, 4.3, 4.4, 5.1_
  - _Boundary: DefaultHtmlShell, ComrakMarkdownRenderer, UiAssets_

- [x] 3.3 AppController から読み取り専用の open-file flow を統合タイトルバー表示へ反映する
  - file selection の success / cancel / failure を既存 viewer state に反映し、single-document のまま表示更新する。
  - 成功時は titlebar のファイル名と本文表示が更新され、cancel 時は現在表示を保持し、failure 時はアプリを終了せず window 内エラーに落とす。
  - 閲覧操作だけでは元の Markdown ファイルが変更されず、編集・保存・複数文書機能が増えない状態で完了とする。
  - _Depends: 3.1, 3.2_
  - _Requirements: 3.3, 3.4, 3.5, 5.1, 5.4, 6.1, 6.4_
  - _Boundary: AppController_

- [x] 3.4 起動直後、空状態、エラー状態でも単一の上部バー体験を保つ
  - runtime prerequisite success 時は empty viewport と統合タイトルバーを表示し、failure 時は原因確認可能な失敗状態を出す。
  - 読み込みエラー時も titlebar を維持したまま本文領域側で error を見せる。
  - 初期表示、文書表示、エラー表示のどれでも二重の上部 UI が現れない状態で完了とする。
  - _Depends: 1.1, 1.2, 2.3, 2.4, 3.1_
  - _Requirements: 1.1, 1.3, 2.3, 3.5, 7.3_
  - _Boundary: SciterRuntime, DefaultHtmlShell, SciterWindow_

- [x] 4. Validation: viewer と配布前提を検証する
- [x] 4.1 タイトルバー shell と interaction script の単体検証を追加する
  - 初期 shell に app identity、open control、disabled future controls、window controls、viewport 構造が入ることを確認する。
  - script が command 名、disabled guard、drag region 判定を正しく扱うことを確認する。
  - 単体テストで shell contract と titlebar command contract の崩れを検出できる状態で完了とする。
  - _Depends: 2.3, 2.4, 2.5_
  - _Requirements: 1.1, 1.2, 2.1, 2.2, 2.3, 3.1, 6.2, 6.3_
  - _Boundary: DefaultHtmlShell, TitleBarInteractionScript_

- [x] 4.2 Windows window chrome と create-window mode の host-level 検証を追加する
  - fake Win32 seam で minimize、maximize/restore、close、begin drag の分岐を固定する。
  - create-window mode が追加の可視 title row を出さない前提を host-level test で固定する。
  - native window 操作と window host 前提の後退をテストで検出できる状態で完了とする。
  - _Depends: 1.2, 2.1_
  - _Requirements: 1.3, 1.4, 1.5, 1.6, 1.7, 7.1_
  - _Boundary: WindowsWindowChrome, SciterWindow_

- [x] 4.3 Window routing と open-file state transition の結合検証を追加する
  - open-file success で file name と整形済み本文が更新され、cancel で状態が維持され、failure で window 内エラーへ落ちることを確認する。
  - window-* command が app controller に侵入せず、open-file だけが application flow を進めることを確認する。
  - viewer-only command boundary と single-document state の維持が結合テストで見える状態で完了とする。
  - _Depends: 3.1, 3.2, 3.3, 3.4_
  - _Requirements: 3.2, 3.3, 3.4, 3.5, 4.1, 4.2, 4.3, 5.1, 5.4, 6.1, 6.4_
  - _Boundary: AppController, SciterWindow, DefaultHtmlShell_

- [x] 4.4 起動前提と配布 runtime の smoke 検証を追加する
  - runtime prerequisite failure で原因確認可能な失敗状態になることを確認する。
  - 必要 runtime file が揃えば開発環境なしで起動できることを確認する。
  - Windows 上の smoke check で配布前提の成立が確認できる状態で完了とする。
  - _Depends: 1.1, 3.4_
  - _Requirements: 7.1, 7.2, 7.3_
  - _Boundary: SciterRuntime, SciterWindow_

- [x] 4.5 単一上部バーと固定スクロールの UI smoke 検証を追加する
  - 起動直後に上部バーが 1 本だけ見えることを確認する。
  - 長文で titlebar 固定と本文スクロールが保たれ、window controls が閲覧内容を壊さないことを確認する。
  - Windows 上の smoke check で最小 viewer 体験の成立が確認できる状態で完了とする。
  - _Depends: 3.1, 3.3, 3.4_
  - _Requirements: 1.1, 1.3, 1.4, 1.5, 1.6, 1.7, 2.1, 2.2, 2.3_
  - _Boundary: DefaultHtmlShell, SciterWindow, WindowsWindowChrome_
  - _Verification: 2026-04-29 Windows 実機で native title bar が消え、統合タイトルバーが 1 本のみ表示され、titlebar 固定・本文スクロール・window controls（drag/minimize/maximize/close）の動作を確認済み。_

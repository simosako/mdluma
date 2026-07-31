# Platform Contract Extraction Gap Analysis

調査日: 2026-07-28

## Analysis Summary

- `FileDialog`、`FontDialog`、`WindowChromeController`、`ExternalEditorLauncher` には再利用可能な契約とフェイクがあるが、契約が Windows 実装ファイルや Sciter native handle に結合しており、platform-neutral な公開面にはなっていない。
- URL opening、settings/log directory 解決、既定 document opening、親 window 位置取得、geometry 保存には共有契約がなく、`app.rs`、`lib.rs`、`settings.rs`、`debug_log.rs`、Sciter FFI に Windows 前提が分散している。
- 最大の統合課題は、共有契約から native handle を排除しつつ modal dialog と window 操作の owner/lifetime を維持すること、ならびに global debug logger の初期化順序を path resolver 注入へ適合させることである。
- 既存のジェネリック DI と近接単体テストは移行の土台として強い。一方、URL opener と path resolver のフェイク、非 Windows compile check、Windows 実機回帰、settings 保存失敗の一貫した表示には追加検証が必要である。
- 実装候補は既存モジュールの最小拡張、新しい集約 platform service、新規契約と既存 controller 拡張を組み合わせる段階的ハイブリッドの3案である。設計フェーズでは責務粒度、owner 表現、error model、logger 初期化を先に確定する必要がある。

## Document Status

- 対象仕様: `platform-contract-extraction`
- 文書言語: 日本語 (`spec.json.language = "ja"`)
- Requirements: 生成済み、**未承認**
- Design / Tasks: 未生成
- 分析方法: core steering、feature brief、requirements、roadmap、既存移植設計、`ccc` semantic search、対象コードの直接読解、Cargo 依存調査、外部 API 資料の照合を使用した brownfield gap analysis
- 注意: requirements 未承認のため、本書は設計入力として利用できるが、要件承認前の実装開始を承認するものではない。
- 検証制約: `cargo check --target x86_64-apple-darwin` を試行したが、このセッションでは `cargo` が `PATH` に存在せず実行できなかった。非 Windows compile gap は静的コード調査に基づく。

## 1. Scope and Boundaries

### In Scope

- file / external editor executable / font dialog の共有契約と Windows adapter
- window minimize、maximize/restore、close、最大化状態、cascade 用位置、geometry 利用
- 許可済み外部 URL の OS browser opening
- settings と debug log の directory resolver
- 設定済み editor と Windows 既定 `notepad.exe` による document opening
- platform implementation を選択する composition root
- 契約フェイク、既存 Windows 動作、起動経路、viewer 状態の回帰確認

### Out of Scope

- macOS/Cocoa adapter の実装と macOS 固有 path/editor/dialog
- Sciter dynamic loader、共通 API table、WndProc、Win32 message の分離
- native-frame UI、Sciter runtime 更新、`.app` packaging、署名、notarization
- settings schema の変更、新しい viewer 機能、macOS no-op adapter

### Adjacent Specifications

- 本仕様は migration-first roadmap の最初の実装仕様であり、dependency はない。
- suspend 中の `macos-sciter-runtime-evidence` は本仕様を block しない。
- 後続の `sciter-win32-separation` が Sciter loader/FFI/Win32 message 境界を所有する。
- その後の `macos-sciter-host-smoke` が macOS host と実 runtime 接続を所有する。
- `minimal-markdown-viewer`、`command-line-file-open`、`drag-and-drop-file-open`、`font-settings`、`external-editor-integration`、`about-dialog` の既存受け入れ動作を変更しない。

## 2. Current State Investigation

### 2.1 Architecture and Dependency Direction

現在の主な依存関係は次のとおりである。

```text
lib.rs (startup/composition)
  -> AppController<D, F, L, R, H, U, S, E>
       -> FileDialog / FontDialog / ExternalEditorLauncher
       -> ViewerUi
       -> settings and platform free functions
  -> SciterWindow
       -> Box<dyn WindowChromeController>
       -> WindowsWindowChrome (constructor hard-code)
  -> WindowsFileDialog / WindowsFontDialog (type alias hard-code)
```

望ましい既存パターンは、trait で境界を定義し、上位入口で具体型を組み立て、テストでは fake/recording implementation を注入する構成である。`AppController` はこのパターンをすでに採用しているが、platform service 全体には適用されていない。

### 2.2 Reusable Assets

| Asset | Existing capability | Reuse assessment |
|---|---|---|
| `src/app.rs` | viewer 状態遷移、成功/cancel/failure 処理、ジェネリック DI | 再利用可能。ただし8型パラメーターと複数の platform 直参照が hotspot |
| `src/platform/windows_file_dialog.rs` | Markdown / `.exe` 選択、cancel/failure 分離、owner 検証、テスト seam | Windows adapter とテストロジックを再利用可能。契約型の移動が必要 |
| `src/platform/font_dialog.rs` | `FontDialog`、`FontDialogResult`、契約テスト | 最も契約分離に近いが、`SciterWindowHandle` 依存が残る |
| `src/platform/windows_font_dialog.rs` | 初期 font family/point size、ChooseFont、cancel/failure | Windows adapter と単体テストを維持可能 |
| `src/platform/windows_window_chrome.rs` | minimize、maximize/restore、close、live handle 検証、Win32 seam | Windows adapter と fake seam を再利用可能。契約型と具体実装の分離が必要 |
| `src/external_editor.rs` | executable + document path の process spawn 契約 | 設定済み editor launcher として再利用可能。OS 既定 opener とは意味を分ける必要あり |
| `src/settings.rs` | schema、default、validation、load/save、fallback path helper | schema と file I/O を維持し、path construction の入力だけを分離可能 |
| `src/debug_log.rs` | debug-only logger、file fallback、stderr fallback | behavior は維持可能。global `OnceLock` と resolver 注入順序が課題 |
| `src/startup_args.rs` / `src/open_paths.rs` | 0/1/2+ path、入力順、最大10件 | 変更不要に近い強い回帰資産 |
| `src/viewer_launcher.rs` | child process と cascade env の受け渡し | platform-neutral な標準 `Command` 実装として再利用可能 |
| `src/sciter/window.rs` | `ViewerUi`、window command routing、`Box<dyn WindowChromeController>` | trait object 利用は再利用可能だが constructor が Windows 実装を固定 |
| `src/sciter/ffi.rs` | geometry capture/restore、screen 判定、cascade offset | Windows behavior の既存実装。後続 Sciter 分離を先取りせず adapter から委譲する境界が必要 |
| `src/errors.rs` | user message と operator diagnostic の分離 | 既存 error 方針を再利用可能。URL/path/window query 用 variant の扱いは未確定 |

### 2.3 Existing Conventions

- crate root の `lib.rs` が主要型を再公開し startup wiring を行う。
- 責務単位の小さな trait と concrete type を使用し、テストは実装近傍へ置く。
- platform failure は `ViewerError` に変換し、user message と operator diagnostic を分離する。
- cancel は error ではなく enum outcome として扱う。
- UI state transition は Rust の `AppController` が所有し、Sciter/JS は command 起点に限定する。
- Windows が既定 target で、`.cargo/config.toml` は `x86_64-pc-windows-msvc` を指定する。
- release build は debug logger をコンパイル時に無効化する。

### 2.4 Coupling Hotspots

| Hotspot | Evidence | Integration concern |
|---|---|---|
| `src/app.rs` | 約4,758行、8個の generic parameter、URL/path/window位置/geometry/editor fallback が混在 | service ごとの generic 追加は constructor とテストをさらに肥大化させる |
| `src/sciter/ffi.rs` | 約2,368行、geometry と Win32 lifecycle が Sciter API と同居 | 本仕様で全面分離すると `sciter-win32-separation` を先取りする |
| `src/sciter/window.rs` | `WindowsWindowChrome` を直接 import/construct | composition root 以外で platform implementation を選択している |
| `src/lib.rs` | Windows concrete types を alias、builder、public export で露出 | shared startup signature が Windows 型名に依存する |
| `src/debug_log.rs` | global `OnceLock` が resolver 未注入の `create_logger` を遅延呼び出し | composition と最初の `debug_log!` の順序が behavior を左右する |

## 3. Requirements Feasibility and Gap Map

### Requirement-to-Asset Map

| Requirement | Existing assets and evidence | Gap classification | Gap / constraint |
|---|---|---|---|
| R1 Platform Service 利用時の一貫性 | `AppController` の DI、`OpenFileResult` / `FontDialogResult`、state error overlay、selected path を表示成功後に commit する処理 (`app.rs:367-398`) | **Missing / Constraint** | 全 service を束ねる platform boundary がない。URL、path、window query は直呼び出し。成功一回/cancel no-op の既存テストは dialog/editor 中心で、全契約を横断する共通検証がない。native handle が shared trait に漏れている |
| R2 Native File・Font 選択 | `FileDialog` (`windows_file_dialog.rs:5-21`)、`FontDialog` (`font_dialog.rs:5-17`)、Windows 実装と cancel/failure tests | **Constraint / Missing** | `FileDialog` contract が Windows file に同居。両 dialog contract が `SciterWindowHandle` を受け取る。非 Windows では Windows 型が error stub として存在し、no-op adapter 禁止方針と整合しない。file failure は伝播するが、font failure は debug log に吸収するという操作別 policy を維持する必要がある |
| R3 Native Window 操作と情報 | `WindowChromeController`、live handle 検証、`Box<dyn ...>`、Win32 seam tests、geometry FFI、cascade launcher | **Missing / Unknown / Constraint** | contract と Windows 実装が同居し raw Sciter handle を公開。`SciterWindow` constructor が Windows 実装を固定。親位置取得が `app.rs:275-297` に `cfg` 付きで存在。geometry capture は Sciter FFI global state。chrome、位置、geometry を一契約にするか分割するか未確定 |
| R4 外部 URL Opening | Sciter hyperlink routing は `http` / `https` のみ command 化 (`sciter/window.rs:520-539`)、Windows `ShellExecuteW` 実装 | **Missing** | opener trait/fake/error がない。`ShellExecuteW` 戻り値を捨て、呼び出しも常に `Ok(())` (`app.rs:251-254`)。非 Windows では `platform::windows_browser` module が未定義になる静的 compile gap。scheme policy の defense-in-depth 配置も要検討 |
| R5 Settings・Debug Log Directory | `settings_file_path` と `default_log_directory`、temp fallback tests、settings schema/roundtrip/default、debug-only logger | **Missing / Constraint** | resolver trait がなく `LOCALAPPDATA` を2箇所で直参照。settings と log の root 解決が重複。logger `OnceLock` へ resolver を注入する順序が未確定。theme/font/recent 等の save failure は無視される箇所があり、R5.5 の「ユーザー操作時は window 内表示」と一貫しない |
| R6 Default Document Opening | `ExternalEditorLauncher`、process spawn、document-loaded UI enablement tests、launch success 後 close、failure overlay、`notepad.exe` tests | **Missing / Constraint** | 設定済み executable launcher はあるが OS default document opener contract はない。fallback 選択が `app.rs:568-570` に Windows 固定。既定 opener と user-selected executable を同一 service にするか分離するか未確定 |
| R7 Startup 動作 | `startup_args.rs`、`open_paths.rs`、`execute_launch_plan`、runtime preflight、最大10件と順序 tests | **Constraint / Missing** | startup behavior 自体は強く実装済み。Windows services は `StartupController` alias と builder に固定され、window chrome は下位 constructor で別途固定。path resolver が `run()` と SpawnChildren で個別生成される。platform selection が単一地点ではない |
| R8 移行範囲と Windows 回帰 | theme/search/selection/DnD/titlebar/window tests と実装、runtime validation、既存 feature inventory | **Unknown / Constraint** | shared unit tests は豊富だが、Windows 10/11 native integration regression matrix と CI test 実行証拠が不足。caption drag と DOM replacement 後の native DnD は本仕様の回帰対象だが、WndProc変更は範囲外。非 Windows compile check を現環境で実行できていない |

### 3.1 Missing Capabilities

1. OS-neutral な platform contract 公開面と Windows implementation namespace。
2. URL opener の `Result` 契約、fake、Windows return-value mapping。
3. application-data/settings/log directory resolver と file I/O の分離。
4. OS default document opener、または既定選択 policy を含む editor opening 契約。
5. 親 window position query と geometry capture/persist の application-level 境界。
6. platform implementation を一度だけ選択する composition root。
7. platform operation failure を表現する error model と、操作別の表示/log/伝播 policy。
8. shared contract から `HWND`、Cocoa 型、Sciter native handle を漏らさない owner/window identity 表現。
9. URL/path/window service の recording fake と contract tests。
10. Windows native integration と非 Windows compile を含む verification matrix。

### 3.2 Existing Behavioral Gaps Exposed by the Requirements

- URL opening failure は検出も伝播もされない。
- `open_url_in_browser` は非 Windows target で参照先 module が存在しない。
- `toggle_theme`、font apply、recent file 更新などは settings save error を無視するため、R5.5 を文字どおり適用すると既存処理との差がある。
- startup child cascade は保存 geometry 不在時に `(100, 100)` を基準にする。一方、R3.7 の `(0, 0)` fallback は「親 window 位置取得失敗時」の drop/cascade 経路に実装済みであり、起動時 fallback と混同しない設計が必要である。
- Windows concrete adapter が非 Windows で「unavailable」error を返す stub としてコンパイルされる。brief は未実装 OS を no-op で偽装しないことを要求しており、platform selection から Windows 型を除外する方が境界に適合する。

### 3.3 Cross-Cutting Non-Functional Constraints

- 軽量性、起動速度、省メモリ、単一 crate を維持する。
- Markdown -> HTML -> Sciter 表示と viewer state transition を変更しない。
- service 成功は一度だけ、cancel は完全な no-op、failure は document と session を保持する。
- user-facing error と operator diagnostic を分離する。
- URL allow policy は shared security boundary として維持し、adapter は許可済み URL の opening に限定する。
- native dialog、window、Sciter UI は UI thread affinity を持つ。contract へ不用意に `Send + Sync` を要求できない。
- `SciterWindowHandle` は raw pointer であり、共有 service を background thread 向けに一般化しない。
- settings schema と既存 JSON の読み書き結果を変更しない。
- 本仕様で Sciter loader/Win32 message の分離を先取りしない。

## 4. External Dependency and Compatibility Findings

### Current Dependencies

- Rust 2021、`rust-version = 1.80`
- Windows dependency: `windows-sys = "0.61"`、lockfile は `0.61.2`
- 有効 feature: `Win32_UI_WindowsAndMessaging`、`Win32_UI_Controls_Dialogs`、`Win32_UI_Shell`、`Win32_Graphics_Gdi`

### Compatibility Assessment

- 現在の feature は `ChooseFontW`、`CHOOSEFONTW`、GDI DPI query、window operations、`ShellExecuteW` に適合する。
- file dialog は `GetOpenFileNameW` 相当の struct/function を手書きしている。抽出のために API を置換する必要はなく、回帰リスクを抑えるなら既存 ABI を Windows adapter 内へ移すだけでも成立する。
- `SHGetKnownFolderPath(FOLDERID_LocalAppData)` を採用する場合、返却 memory 解放用に `CoTaskMemFree` と `Win32_System_Com` feature が必要になる。ただし `%LOCALAPPDATA%` override の既存挙動が変わる可能性がある。
- `ShellExecuteW` は32より大きい戻り値を成功、32以下を failure として扱う必要がある。現在は戻り値を捨てている。
- `ShellExecuteW` と shell extension 利用では COM apartment 初期化状態が設計課題になり得る。Sciter/UI thread の既存 COM 初期化状態を Windows 実機で確認する必要がある。
- `std::process::Command` は shell を介さず executable と document path を別引数で渡しており、設定済み editor の既存用途に適合する。
- 新規外部 crate は現時点で必須ではない。

### Toolchain Constraint

- `.cargo/config.toml` が Windows MSVC target を既定指定する。
- macOS host で shared unit tests を実行するには target override と非 Windows compile path の成立が必要である。
- 今回の環境では `rtk cargo` は process spawn に失敗し、raw `cargo` も command not found だったため、build/test の実証は design/implementation phase に持ち越す。

## 5. Implementation Approach Options

### Option A: Extend Existing Components Minimally

#### Outline

- 既存の `platform` module layout を維持する。
- `FileDialog`、`WindowChromeController` などの trait を既存ファイルから最小限移動、または re-export 面だけ整理する。
- URL opener、path resolver、default opener を小さな個別 trait として追加する。
- `AppController` の既存 generic DI に必要な service を追加する。
- `SciterWindow` の constructor へ window chrome を渡せる既存 private seam を公開範囲内で利用する。

#### Likely Touchpoints

- `src/platform/mod.rs`
- `src/platform/windows_file_dialog.rs`
- `src/platform/font_dialog.rs`
- `src/platform/windows_window_chrome.rs`
- `src/app.rs`
- `src/lib.rs`
- `src/settings.rs`
- `src/debug_log.rs`
- `src/sciter/window.rs`

#### Trade-offs

- 利点: 新規ファイルと移動量が少なく、Windows implementation の回帰差分を抑えやすい。
- 利点: 既存 generic/fake pattern を直接利用できる。
- 欠点: Windows 名前の module と shared contract の混在が残りやすい。
- 欠点: `AppController` の generic parameter と constructor がさらに増える。
- 欠点: native owner と geometry 境界が場当たり的になる可能性がある。

#### Estimate

- Effort: **M-L (3日〜2週間)**。契約追加自体は小さいが、logger、geometry、回帰テストが広い。
- Risk: **Medium-High**。差分は小さい一方、依存方向の不完全な改善と generic 複雑化が残る。

### Option B: Create a New Aggregated Platform Layer

#### Outline

- `src/platform/contracts.rs` に OS-neutral value/result types と contract を集約する。
- `src/platform/windows/` に dialogs、browser、paths、document opener、window service を整理する。
- `PlatformServices` のような一つの facade、または composition bundle が operations を提供する。
- `lib.rs` が唯一の platform selection point となり、controller/UI へ選択済み service を渡す。
- shared code から Windows concrete type の public export を除去する。

#### Responsibility Boundary

- contracts: OS-neutral input/output と failure contract のみ。
- Windows adapter: raw handle conversion、Win32 API、`LOCALAPPDATA`、`notepad.exe` policy。
- application: viewer state transition、cancel/failure presentation、URL allow policy。
- Sciter boundary: opaque runtime window identity の取得と platform adapter への受け渡し。

#### Trade-offs

- 利点: 依存方向と namespace が明確で、後続 macOS adapter の差し込み点が理解しやすい。
- 利点: service bundle により `AppController` の generic parameter 増加を抑えられる可能性がある。
- 利点: contracts と Windows integration tests を分離しやすい。
- 欠点: ファイル移動と import/re-export 差分が大きく、Windows regression surface が広い。
- 欠点: facade が大きすぎると Interface Segregation を損ない、不要な service を各 consumer が参照できる。
- 欠点: Rust 1.80 では trait upcasting に依存しない明示的な構成が必要。

#### Estimate

- Effort: **L (1〜2週間)**。module relocation、composition、consumer 更新、回帰確認を含む。
- Risk: **High**。native owner、service object design、logger lifecycle を同時に再設計するため。

### Option C: Phased Hybrid Extraction

#### Outline

- Phase 1: OS-neutral contracts/value types を新設し、既存 Windows code は内部実装をほぼ変えず adapter として適合させる。
- Phase 2: URL、path、default opener を contract 化し、`SettingsFile` と logger は resolved path を受け取る構成へ限定的に変更する。
- Phase 3: `lib.rs` で service bundle を組み立て、`SciterWindow` の Windows chrome hard-code と `AppController` 内の platform `cfg` を除去する。
- Phase 4: shared contract/fake tests と Windows regression tests を追加し、既存 module の directory relocation は必要性に応じて実施する。

#### Combination Strategy

- 新規作成: `contracts`、path/url/default opener contract、recording fakes、composition bundle。
- 既存拡張: Windows dialog/chrome/browser implementations、`SettingsFile` constructor、debug logger initialization、startup builder。
- 既存維持: Markdown rendering、HTML shell、viewer state model、startup path planning、Sciter loader/Win32 message internals。

#### Trade-offs

- 利点: 契約と behavior preservation を別ステップで検証できる。
- 利点: Windows API 実装を大きく書き換えず依存方向を改善できる。
- 利点: 後続 `sciter-win32-separation` との境界を保ちやすい。
- 欠点: 一時的に旧/新 module layout や adapter forwarding が併存する可能性がある。
- 欠点: phase boundary を明確にしないと中間状態が恒久化する。
- 欠点: Option A より planning と test matrix が複雑になる。

#### Estimate

- Effort: **L (1〜2週間)**。広い integration surface は変わらないが、段階検証で再作業を抑えられる。
- Risk: **Medium-High**。主要 unknown は残るが、Windows implementation の置換を避けて隔離できる。

## 6. Research Needed for Design Phase

1. **Native owner/window identity**: shared contract から `SciterWindowHandle` を除去しながら、Windows modal owner と live-window validation をどこで行うか。window-bound service、project opaque token、UI/platform bridge の比較が必要。
2. **Service granularity**: 個別 trait、単一 facade、data-only service bundle のどれが `AppController` の generic 数、testability、Rust 1.80 compatibility のバランスに優れるか。
3. **Window boundary**: custom chrome、parent position、geometry capture/persist を一契約にまとめるか分割するか。Sciter FFI からの委譲方法は後続 WndProc 分離を先取りしないこと。
4. **Logger initialization**: resolver を `OnceLock` 初期化前に供給する API、最初の `debug_log!` より前の composition、テスト process 内での再初期化不能性をどう扱うか。
5. **Windows path resolution semantics**: `%LOCALAPPDATA%` 環境変数を維持するか `SHGetKnownFolderPath` を採用するか。要件は既存保存先・結果維持を優先するため behavior comparison が必要。
6. **URL opener and COM**: `ShellExecuteW` return mapping、COM apartment ownership、allowed scheme の二重検査位置を Windows 実機で確認する。
7. **Error model**: URL、directory、window query、default opener failure を既存 `ViewerError` variant に寄せるか、platform operation variant を追加するか。user-facing/error overlay/debug-only の operation-specific policy を表にする。
8. **Settings save failures**: R5.5 を theme/font/recent/geometry など全 user operation に適用するか、既存明示表示箇所だけを維持するか。requirements 承認前に意図を確認する。
9. **Default opener semantics**: Windows の `notepad.exe` fallback を platform policy とするか、既定 document opener operation の Windows 実装とするか。設定済み executable launcher との責務分離が必要。
10. **Verification environment**: Windows 10/11 GUI VM/実機、MSVC toolchain、debug/release logger、native dialogs、browser association、Notepad、DWM corner、caption drag、WM_DROPFILES を検証できる環境を確定する。
11. **Non-Windows compile target**: platform contracts 抽出後に Windows concrete module が非 Windows build graph へ入らないことを `cargo check --target x86_64-apple-darwin` または `aarch64-apple-darwin` で確認する。

## 7. Verification and Regression Considerations

### Shared Automated Tests

- 各 contract fake で success exactly once、cancel no-op、failure state preservation/error propagation を確認する。
- URL opener fake で許可済み URL のみ一度渡され、失敗しても document/state が維持されることを確認する。
- path resolver を filesystem writing と独立して test し、LOCALAPPDATA 成功と temp fallback を確認する。
- settings schema、legacy/default/invalid JSON、roundtrip の既存 tests を維持する。
- configured editor、Windows default editor policy、launch failure、close request failure の順序を確認する。
- startup 0/1/2〜10/11+ arguments、入力順、cascade offsets を維持する。
- parent position failure が `(0, 0)` を返す recording test を追加する。
- non Windows target の compile-only check を追加する。

### Windows Integration / Manual Regression

- Markdown file dialog filter、external editor `.exe` filter、cancel、native failure。
- font initial family/point size、confirm、cancel、failure debug log。
- minimize、maximize/restore、close、DWM corners、invalid handle behavior。
- `%LOCALAPPDATA%\MDLuma\settings.json` と `logs\`、temp fallback、debug/release 差異。
- browser association success/failure と viewer continuation。
- configured editor と `notepad.exe` fallback、成功時のみ viewer close。
- saved geometry restore/capture、親 window cascade、画面外 geometry handling。
- custom titlebar caption drag、theme、search、selection/copy、recent files。
- DOM replacement 後の Sciter exchange DnD と `WM_DROPFILES` fallback。
- runtime/asset preflight failure で viewer が表示されないこと。

## 8. Complexity and Risk Assessment

- Overall Effort: **L (1〜2週間)**。既存実装の再利用性は高いが、6種類以上の OS service、startup composition、settings/logger、window/Sciter integration、Windows regression を横断する。
- Overall Risk: **High**。native handle と UI thread affinity、global logger lifecycle、Sciter FFIとの責務境界、非 Windows compile path、実機依存 regression が同時に存在する。
- Dependency Risk: **Low**。現在の `windows-sys 0.61.2` で主要 API を満たし、新規 crate は必須でない。
- Behavioral Regression Risk: **Medium-High**。機能追加ではないが file/font/window/editor/settings/startup の既存経路を広く再配線する。
- Performance Risk: **Low-Medium**。service calls 自体は軽いが、heap allocation/dynamic dispatch や startup resolver 初期化を過剰に増やさない設計が必要。

## 9. Design-Phase Guidance

最も情報価値が高い設計順序は次のとおりである。これは実装案の最終決定ではない。

1. OS-neutral contract に許可する value type と native owner/window identity の境界を定義する。
2. service 粒度と composition bundle の形を決め、`AppController` の generic 増加上限を確認する。
3. operation ごとの success/cancel/failure、error propagation、user display、debug logging policy を表にする。
4. settings/log resolver と debug logger 初期化 sequence を決める。
5. Sciter FFI の geometry/chrome behavior を変更せず委譲する境界を決める。
6. Windows adapter の既存 API behavior を固定する characterization tests を先に揃える。
7. shared compile/test と Windows 10/11 integration regression の verification matrix を設計へ組み込む。

Option C は既存 Windows implementation を大きく変更せず、契約・composition・回帰確認を段階化できるため有力な設計候補である。ただし owner/window identity と logger lifecycle の解決なしに採用を確定すべきではない。

## 10. Assumptions and Open Questions

- 本仕様では macOS adapter を実装せず、non Windows target では未選択 platform implementation を no-op で偽装しない。
- Windows の file/font dialog API、DPI calculation、cascade constants、DWM corner behavior は抽出時に変更しない。
- `%LOCALAPPDATA%` の既存保存結果を最優先する限り、Known Folder API への置換は必須ではない。
- `ViewerUi::native_window_handle()` 自体を後続仕様まで残す可能性はあるが、その型を shared platform contract の operation parameter として公開し続けることは brief と不整合である。
- R5.5 の settings save failure 表示範囲は requirements 未承認段階で確認が必要である。
- Windows native regression を自動化できない部分は、明示的な手動/VM evidence として設計する必要がある。

## References

- `.kiro/specs/platform-contract-extraction/requirements.md`
- `.kiro/specs/platform-contract-extraction/brief.md`
- `.kiro/steering/product.md`
- `.kiro/steering/tech.md`
- `.kiro/steering/structure.md`
- `.kiro/steering/roadmap.md`
- `.kiro/steering/implemented-features.md`
- `design/initialdesign.md`
- `design/macos-porting-architecture.md`
- Microsoft GetOpenFileNameW: <https://learn.microsoft.com/en-us/windows/win32/api/commdlg/nf-commdlg-getopenfilenamew>
- Microsoft ShellExecuteW: <https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecutew>
- Microsoft SHGetKnownFolderPath: <https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath>
- Rust dyn compatibility: <https://doc.rust-lang.org/reference/items/traits.html#dyn-compatibility>
- Rust `Command`: <https://doc.rust-lang.org/std/process/struct.Command.html>

## Next Steps

1. requirements の未承認点、特に settings save failure 表示範囲をレビューする。
2. 本分析の Research Needed を設計論点として `/kiro-spec-design platform-contract-extraction` を実行する。
3. requirements を同時に承認して進める場合は `/kiro-spec-design platform-contract-extraction -y` を使用する。

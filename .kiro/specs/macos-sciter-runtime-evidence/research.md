# Implementation Gap Analysis: macos-sciter-runtime-evidence

分析日: 2026-07-26

## Status Notice

gap分析時点では要件が未承認だった。technical design full discoveryで見つかったOS matrixの要件ギャップについて、Go判定は検証時点の現行Apple Silicon macOSに限定し、未実行の過去macOS versionを検証済みとしない方針が確認された。

## Analysis Scope and Method

- `.kiro/specs/macos-sciter-runtime-evidence/requirements.md`のRequirements 1-8を既存資産へ対応付けた。
- `product.md`、`tech.md`、`structure.md`、`roadmap.md`、`design/macos-porting-architecture.md`の制約を確認した。
- `ccc search`でruntime loader、API table、window lifecycle、popup、既存テストを特定し、関連ファイルを限定して読んだ。
- 公式Sciter GitLabの固定commitとローカル`libsciter.dylib`を調査し、ハッシュ、Mach-O情報、依存、署名、API smokeの既知証拠を整理した。
- 実装方式は既存拡張、新規ツール、hybridの3案で比較した。ここでは選択を確定しない。

## Executive Summary

- runtime成果物の取得、Git追跡、SHA-256、Universal Binary、native arm64での基本API smokeに必要な前提は概ね揃っている。
- 製品側のSciter loaderとwindow/event-loop経路はWindows専用であり、Phase 0要件を製品コードの小変更だけで満たすことはできない。
- 既存bindingsによる限定ABI smoke、`menu.popup`とwindow lifecycleの100回harness、10秒timeout、append-only証拠、Go/No-Go集約は新規に必要である。
- 6.0.3.18にはmacOS popup/window lifecycle関連の修正履歴があり、描画・破棄完了の観測方法が設計上の高リスク項目である。macOS 12は既知リスクとして残るが、現行要件のmandatory matrixには含めない。
- `libsciter.dylib`の同梱再配布とDeveloper ID再署名は現行EULAだけでは確定できず、Sciter提供元の公開文書または書面回答が外部Go/No-Go blockerである。

## Current State

### Runtime Artifact

- 配置先: `vendor/sciter-js-sdk-main/bin/macosx/libsciter.dylib`
- 公式repository: `https://gitlab.com/sciter-engine/sciter-js-sdk`
- 固定commit: `e31ec0f726bdbe5d0402ad647f3b34feef84654e`
- SHA-256: `be5ac8b83fd46a17b9f6507d38b37ec5c3dcc14466bc36c04f42014d2d506c4b`
- architecture: `x86_64`, `arm64`
- minimum OS: 両sliceともmacOS 11.5
- install name: `/usr/local/lib/libsciter.dylib`
- dependencies: Apple system frameworks、`libc++`、`libSystem`、`libobjc`のみ。非system dylibへの直接依存は見つからない。
- signature: ad-hoc、Team IDなし、entitlementsなし。単独dylibのGatekeeper評価は配布可能性を証明しない。
- 確認済み独立smoke: native arm64でruntime load、`SciterAPI`解決、non-null API table、API version 10、engine version 6.0.3.18。
- 未確認: 既存Rust bindings型経由のABI smoke、popup/window lifecycle、再配布・再署名許諾。

### Existing Runtime Boundary

- `src/sciter/runtime.rs:14-33`: `RuntimePrerequisites`は`sciter_dll_path`とWindows名を持つ。
- `src/sciter/runtime.rs:136-158`: prerequisite、API load、version query、major/minor互換性確認は再利用可能。
- `src/sciter/runtime.rs:36-108`: missing/API unavailable/version mismatchの型付きerrorと利用者・operator向け診断がある。
- `src/sciter/ffi.rs:317-347`: `SciterApi::load()`はWindowsでのみ動作し、non-Windowsは常に`ApiUnavailable`を返す。
- `src/sciter/ffi.rs:191-217`: API tableから抽出する`SciterApiBindings`は`cfg(windows)`である。
- `src/sciter/generated_sciter_bindings.rs:3-7`: 6.0.3.18とAPI version 10の定数がある。
- `src/sciter/generated_sciter_bindings.rs:925-930`: `ISciterAPI`先頭に`version`と`SciterVersion`がある。
- bindings生成は`tools/bindgen/generate_sciter_bindings.ps1`だけで、`-DWINDOWS`を使用しlayout testsを無効化している。

### Existing UI and Lifecycle Assets

- `src/ui/app.js:1192-1198`: 現行機能が`anchor.popup(menu, ...)`を使用する。
- `src/ui/mod.rs`にはpopup呼び出し形状のNode.js asset testがあるが、実Sciter描画やnative lifecycleは検証しない。
- `src/sciter/window.rs:909-939`: non-Windows event loopは明示的errorである。
- Windows event loopにはclose後のgraceful shutdownを検証するseamとunit testがある。
- `src/html_shell.rs:111-114`: EULAが要求するSciter attributionに対応する既存About文字列がある。
- `.github/workflows/release.yml:25-35`: Windows配布物にはSciter `LICENSE`とEULAを同梱している。

### Build and Environment Constraints

- `.cargo/config.toml`は既定targetを`x86_64-pc-windows-msvc`へ固定する。
- `Cargo.toml`にmacOS loader用dependencyや専用binary targetはない。
- `build.rs`のmacOS側はgit commit hash設定だけで、runtime copyやharness buildを行わない。
- 現在のApple Silicon shellでは`uname -m`は`arm64`だが、`rustc`が`PATH`にない。Rust harnessを採用する場合はtoolchain準備が実行前提となる。
- requirementsは製品macOS loaderと`.app` packagingを対象外としているため、これらの不足をPhase 0で解消する必要はない。

## Requirement-to-Asset Map

| Requirement | Existing Assets | Gap Classification | Missing Capability / Constraint |
| --- | --- | --- | --- |
| R1 Runtime同一性 | dylib配置、Git追跡、既知commit/hash、macOS標準`shasum` | Partial / Missing automation | 固定manifest、再実行可能なhash比較、取得元と結果の記録 |
| R2 Apple Silicon条件 | Universal Binary、native arm64 host、`lipo`/`otool`/`codesign` | Partial | process architecture、minOS、依存、install name、署名を統一形式で採取・判定する処理 |
| R3 Runtime API | 独立smoke実績、`SciterVersion`型、version check/error patterns | Missing in repository | 固定runtimeだけを対象にしたnative arm64 smoke、API version 10の明示検証、診断出力 |
| R4 ABI smoke | committed `ISciterAPI`、version定数、Windows API table lookup pattern | Missing | 既存bindings型を実際に使うmacOS smoke、同revision headersとの定数比較、検証範囲の限定表示 |
| R5 Stability | 実製品の`menu.popup`呼び出し、window/event handlerコード、JS shape test | Missing / High risk | 実Sciter host、表示・終了完了観測、各100回、1 cycle 10秒、process crash/hang/heap corruption捕捉 |
| R6 License | repository内LICENSE/EULA、About attribution、Windows配布時のlicense同梱 | Unknown / External blocker | dylib再配布とDeveloper ID再署名の権威ある許諾、根拠記録、遵守可能性判定 |
| R7 Evidence | `review/startup-measurement-v0.3.3.md`の実測記録pattern、typed diagnostics | Missing | append-only結果、環境・runtime・手順・終了状態・診断を比較可能に残すschema/report |
| R8 Go/No-Go | roadmapの依存順とNo-Go時6.0.4.8方針 | Missing | R1-R7集約、Blocked/fulfilled状態、No-Go理由とfallbackを記録する判定表 |

## Key Integration Challenges

### 1. Phase 0と製品macOS Loaderの境界

製品の`SciterApi::load()`をmacOS対応するとPhase 2/3の責務へ踏み込む。Phase 0はruntime採用判断が目的なので、独立harnessなら既存Windows経路を変更せず証拠を得られる。一方、独立harnessが製品と異なるABI宣言を持つとR4を満たさないため、少なくともcommitted `ISciterAPI`型を直接利用する経路が必要である。

### 2. Windows向けBindingsの限定利用

現bindingsは`-DWINDOWS`で生成され、macOS全APIのABI保証には使えない。ただし要件はAPI table先頭の`version`と`SciterVersion`だけを対象とする。この限定範囲をharnessとreportの両方で明示し、成功を全table互換へ拡大解釈しない必要がある。同revision headersは通常buildには不要だが、R4の比較証拠には一時取得またはローカルSDK配置が必要である。

### 3. Sciter Event LoopとLifecycle

macOSで`SCITER_APP_INIT`、window生成、event processing、destroy完了、`SCITER_APP_SHUTDOWN`をどの順序で扱うかは現コードから再利用できない。WindowsのWndProc/message loopを模倣してはならない。公式macOS sample/headerを根拠に最小hostのlifecycleを確定する必要がある。

### 4. Popup完了の観測

`popup()`から戻ることと画面表示完了は同義ではない。100回testでは、表示完了、close要求、close eventまたは無効化までを1 cycleとして観測する必要がある。自動操作が実UI lifecycleを通ることを保証しつつ、各cycleを10秒でsuperviseする仕組みが必要である。

### 5. CrashとHeap Corruptionの捕捉

通常のin-process testはcrashした時点でreportを残せない。親processによる子harness監視ならsignal、exit status、timeoutを記録できるが、heap corruptionの非crash検出にはGuard Malloc、AddressSanitizer、malloc diagnostics等の適用可否を調査する必要がある。Sciter dylib内部まで検出できる範囲を設計で明示する必要がある。

### 6. macOS Version Matrix

6.0.3.18 changelogはmacOS 12のpopup AVを`attempt to fix`と記す。Go判定は検証時点の現行Apple Silicon macOSに限定し、未実行のmacOS 12を検証済みとして扱わない。将来macOS 12をsupport matrixへ加える場合は再検証が必要である。

### 7. License Gate

EULAは製品定義に`sciter.dylib`を含む一方、利用許諾本文は`sciter.dll`を名指しする。公開文書だけではmacOS dylibの同梱、install-name変更、Developer ID再署名を肯定できない。技術実装で解消できないため、提供元回答が得られなければ要件どおりNo-Goとなる。

## Implementation Approach Options

### Option A: Extend Existing Product Runtime Boundary

`src/sciter/ffi.rs`と`runtime.rs`へmacOS loaderとAPI table経路を追加し、その経路でevidence testを実行する。

**Potential changes**:
- `SciterApi::load()`のmacOS branch
- runtime-neutralなfile名とerror用語
- `ISciterAPI`のmacOS可視化
- `SciterWindow`のmacOS event loop/lifecycle

**Advantages**:
- smokeと後続製品実装が同じコードを使う。
- typed errors、version model、既存testsを再利用しやすい。

**Disadvantages**:
- Phase 0の範囲を超え、Phase 2/3を先取りする。
- 2,000行超の`ffi.rs`と1,900行超の`window.rs`に変更が集中し、Windows回帰面が広い。
- runtimeがNo-Goだった場合に不要な製品変更が残る。

**Effort / Risk**: L / High。Sciter共通化とmacOS lifecycleを同時に扱うため。

### Option B: New Standalone Evidence Toolkit

製品crateとは分離された小さな検証tool群を`tools/`配下に置き、metadata probe、API/ABI smoke、UI lifecycle harness、supervisor、evidence reportを独立実行する。

**Potential components**:
- binary metadata収集script
- committed bindingsを限定的に取り込むnative arm64 smoke
- 最小Sciter hostとpopup/window lifecycle fixture
- timeout/crash監視runner
- append-only evidence reportとGo/No-Go checklist

**Advantages**:
- Phase 0境界とWindows無変更方針に最も適合する。
- runtime No-Go時に製品コードを巻き戻す必要がない。
- 各検証を個別に再実行・診断しやすい。

**Disadvantages**:
- 製品loaderとは別コードになる。
- bindingsの取り込み方を誤ると製品ABIの証拠にならない。
- toolchainと実行手順を別途管理する必要がある。

**Effort / Risk**: M-L / Medium-High。API probeは小さいがUI lifecycleとcrash検出が難所。

### Option C: Hybrid Evidence Toolkit with Reusable ABI Seam

metadata、supervisor、report、UI fixtureは独立toolとして新設し、既存bindingsのversion/API table先頭を利用する最小ABI seamだけを共有可能な形で切り出す。製品macOS loaderやwindow adapterは実装しない。

**Potential split**:
- 新規: evidence manifest/report、metadata probe、process supervisor、UI fixture
- 限定共有: `ISciterAPI`先頭を用いるAPI/version smoke
- 既存再利用: version constants、About attribution、typed diagnostic conventions、roadmap dependencies

**Advantages**:
- Option Bの隔離性を維持しつつ、R4が既存bindingsと無関係になることを防げる。
- Phase 2が採用できる検証済みの最小ABI知見を残せる。
- 製品のWindows runtime pathを変更しない。

**Disadvantages**:
- 共有seamの置き場所を誤るとPhase 2の先取りになる。
- 一時的なharnessと将来のloaderの重複を許容する必要がある。
- Option Bより設計判断が増える。

**Effort / Risk**: M-L / Medium。境界を限定できれば最もバランスがよいが、event loopの外部リスクは残る。

## Complexity and Risk Assessment

| Area | Effort | Risk | Reason |
| --- | --- | --- | --- |
| Artifact identity / metadata | S | Low | macOS標準toolと既知期待値で実行可能 |
| API + limited ABI smoke | M | Medium | native dynamic loadと既存Windows生成bindingsの限定利用が必要 |
| Popup/window stability harness | L | High | event loop、表示・destroy観測、100回、timeout、crash検出が必要 |
| Evidence report / decision matrix | S-M | Low-Medium | 既存review patternはあるがschemaとappend方針が新規 |
| License confirmation | Calendar-dependent | High | 外部権利者回答に依存し、コードで解決不能 |
| Overall | L (1-2 weeks excluding external response) | High | UI lifecycleとlicense gateがcritical path |

## Research Needed for Design

1. **Sciter macOS lifecycle**: 同revisionのheaders/samplesで`SCITER_APP_INIT`、window lifecycle、event loop、shutdownの正しい順序を確認する。
2. **Popup observability**: `Element.popup()`の表示完了と`PopupWindow.close()`/close eventの確実な観測方法を確認する。
3. **OS evidence scope**: 実行macOS versionを判定結果へ明示し、別versionへ結果を一般化しないreport表現を決める。
4. **Heap corruption evidence**: Guard Malloc、ASan、MallocStackLogging等のどれがSciter dylibを含むharnessへ適用可能かを実験する。
5. **Bindings provenance**: 6.0.3.18 headersを一時取得し、committed bindingsの定数と先頭layoutを比較する再現手順を決める。
6. **Evidence format**: Markdownのみ、JSON + generated Markdown、または両方のどれをsource of truthとするかを決める。
7. **Failure isolation**: 各cycleごとのchild processか、100 cycle単位のchild processかを決め、10秒timeoutと診断保持を両立する。
8. **License authority**: Sciter提供元へmacOS dylibの同梱再配布、Developer ID再署名、必要ならinstall-name変更を問い合わせ、回答を保存する。
9. **Toolchain**: 現macOS環境にRust toolchainがないため、Rust harnessを選ぶ場合の導入・version固定手順を決める。

## Design-Phase Guidance

- Phase 0のGo/No-Goという境界を守るには、Option BまたはOption Cを中心に比較する価値が高い。
- API/ABI smokeとUI lifecycle harnessは分離し、API smoke成功をpopup/window安定性の代替にしない。
- supervisorはharness外に置き、crash、signal、timeoutでも証拠を残せる構成を検討する。
- reportには「確認済み」と「未確認」を明示し、限定ABI smokeを全API互換として扱わない。
- license回答はimplementation taskの明示的blocked inputとして管理する。macOS 12はmandatory gateにせず、未検証riskとして記録する。
- design生成時に`-y`でRequirements 1-8を承認し、full discoveryで試験matrixとlicense blockerを設計判断へ反映する。

---

# Design Discovery & Decisions

## Summary

- **Feature**: `macos-sciter-runtime-evidence`
- **Discovery Scope**: Complex Integration / Full Discovery
- **Key Findings**:
  - 公式6.0.3.18では`SciterExec`によるINIT、LOOP、SHUTDOWNと、`SciterWindowExec`によるwindow state制御を利用できる。
  - popup cycleは`popup-ready`を表示完了、`popup-dismissed`を終了完了として観測でき、tight loopではなくevent-drivenに進める必要がある。
  - Phase 0は製品loaderを変更せず、独立Rust harness、親process supervisor、immutable run evidenceで完結できる。
  - 既存bindingsをversion/API table先頭のABI smokeへ直接使用するが、成功範囲を全APIへ拡張しない。
  - Sciter提供元の再配布・再署名回答は、コードでは解消できないGo/No-Go前提である。判定対象OSは実行時に記録された現行macOSへ限定する。

## Research Log

### Sciter Application Lifecycle

- **Context**: popupとwindow lifecycleを製品macOS loaderなしで実行する手順が必要だった。
- **Sources Consulted**: 公式6.0.3.18 `include/sciter-x-def.h`、`include/sciter-x-window.hpp`、`include/sciter-main.cpp`、`demos/usciter/usciter.cpp`。
- **Findings**:
  - 正式な順序はruntime load、`SCITER_APP_INIT`、window作成、callback登録、HTML load、`SCITER_APP_LOOP`、window destroy、`SCITER_APP_SHUTDOWN`である。
  - `SC_ENGINE_DESTROYED`はwindowの最終notificationであり、destroy完了のnative観測点にできる。
  - runtime handleの途中`dlclose`安全性は確認できないため、harness process終了まで保持する。
- **Implications**: lifecycleをPhase 0専用harness内に閉じ、製品`SciterWindow`やWindows message loopを再利用しない。

### Popup Completion Contract

- **Context**: `popup()`復帰だけでは実際の表示・破棄を証明できない。
- **Sources Consulted**: 公式6.0.3.18 DOM Element docs、out-of-canvas docs、Context7 Sciter.js SDK index。
- **Findings**:
  - `Element.popup()`は`PopupWindow`を返し、`close()`、`isValid`、`close` eventを提供する。
  - 現行MDLumaの呼出し形状は`anchor.popup(menu, ...)`であり、要件中の`menu.popup`はこのmenu popup機能を指す。検証fixtureも同じ形状へ固定する。
  - `popup-ready`を表示完了、`popup-dismissed`を破棄完了として利用できる。
  - `Element.post()`で次のevent-loop iterationへcloseを送ることで、実際にevent pumpを進めたcycleになる。
- **Implications**: fixtureはevent-driven state machineとし、100 cycleの各shown/closed eventをnative supervisorへ送る。

### Failure Supervision and Heap Diagnostics

- **Context**: in-process crashやhangでも失敗cycleと診断を残す必要がある。
- **Sources Consulted**: Apple Malloc Debugging、Crash Reports、Xcode Diagnostics、Clang AddressSanitizer資料。
- **Findings**:
  - 親Rust processが子harnessのstdout/stderr、exit code、signal、timeoutを監視する方式が追加dependencyなしで実現できる。
  - 通常profileでは`MallocScribble`と`MallocCheckHeap*`を子processだけに設定し、allocatorが検出したcorruptionをabortとして捕捉できる。
  - Guard MallocとASanはprebuilt Sciter内部を網羅せず、負荷も大きいため主判定ではなく異常再現profileに限定する。
  - crash report不在はcrashなしの証明にならない。exit/signal/stderrを一次証拠とする。
- **Implications**: supervisorはcycle progressを10秒deadlineで監視し、timeout時はkill後に必ずwaitする。診断profileの限界もreportへ残す。

### Evidence and License Boundary

- **Context**: 自動検証と法的判断を同じ処理で偽装しない必要がある。
- **Sources Consulted**: 公式6.0.3.18 `LICENSE`、`SCITER-ENGINE-EULA.md`、既存About attribution、Windows release workflow。
- **Findings**:
  - BSD文書とEULA表示要件は確認でき、既存About文字列は要求表記に対応する。
  - macOS dylibの同梱再配布とDeveloper ID再署名は公開EULAだけでは確定できない。
- **Implications**: `license-evidence.txt`は人手で権威ある根拠を記録する入力とし、`permitted`以外または根拠欠落ならDecision EvaluatorはNo-Goにする。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Outcome |
| --- | --- | --- | --- | --- |
| Product Extension | 製品`SciterApi`と`SciterWindow`をmacOS対応して検証 | 後続実装と同じcode path | Phase 2/3を先取りしWindows回帰面が広い | Rejected |
| Separate Tool Crate | `tools/`に独立Cargo crateを追加 | dependency管理とserde利用が容易 | single-crate方針に反しworkspace化を誘発 | Rejected |
| Standalone Evidence Toolkit | `rustc`でbuildするstd-only Rust modulesとHTML fixture | 製品src無変更、No-Go時の撤退容易、単一crate維持 | 小さなreport serializerとFFI boundaryが必要 | Selected |
| Shell-only Harness | shellとmacOS commandだけで監視 | 初期実装が短い | GUI event、signal分類、10秒cycle監視、ABI smokeが弱い | Rejected |

## Design Decisions

### Decision: Phase 0専用Standalone Toolkit

- **Context**: runtime採用判断前に製品macOS実装を開始してはならない。
- **Alternatives Considered**:
  1. 製品runtime boundaryを拡張する。
  2. 新しいCargo crateを追加する。
  3. std-only Rust sourceを`rustc`でbuildする。
- **Selected Approach**: `tools/macos-runtime-evidence/`にtoolを置き、製品`src/`と`Cargo.toml`を変更しない。
- **Rationale**: single-crate、軽量性、Windows無回帰、Phase 0の撤退可能性を同時に満たす。
- **Trade-offs**: 後続product loaderとのcode重複を許容する。toolのFFIは証拠取得だけに限定する。
- **Follow-up**: native arm64 Rust toolchainを実行前提として整備する。

### Decision: Typed Child Protocol and External Supervisor

- **Context**: crash/hang時もcycle番号と診断を保存する必要がある。
- **Alternatives Considered**:
  1. 1 process内で検証とreportを行う。
  2. shell timeoutで子processを監視する。
  3. Rust parentがtyped line protocolを監視する。
- **Selected Approach**: 同じtool binaryをmode別childとして起動し、親がshown/closed/created/destroyed eventを監視する。
- **Rationale**: 10秒cycle timeout、signal、exit code、stderrを一元管理できる。
- **Trade-offs**: protocol parserとreader threadが必要になる。
- **Follow-up**: protocol外のstdoutはdiagnosticとして保存し、判定eventとして解釈しない。

### Decision: Immutable Run Directories

- **Context**: 再実行時に前回証拠を保持し比較可能にする必要がある。
- **Alternatives Considered**:
  1. 単一Markdownへ追記する。
  2. JSON databaseを管理する。
  3. UTC run IDごとのimmutable directoryを作る。
- **Selected Approach**: `evidence/runs/<run-id>/`へraw metadata、events、stderr、summaryを保存し、`decision.md`だけを最新集約として更新する。
- **Rationale**: databaseと追加crateを不要にし、失敗artifactをそのまま監査できる。
- **Trade-offs**: run間比較はsummary tableで行い、自動query機能は持たない。
- **Follow-up**: run ID衝突をfail-fastで扱い、既存runを上書きしない。

### Decision: Limited ABI Claim

- **Context**: existing bindingsはWindows defineで生成され、全API互換を証明できない。
- **Selected Approach**: committed bindings型でAPI version fieldと`SciterVersion` entryだけを読む。
- **Rationale**: Requirements 4の範囲とarchitecture designの事前smoke条件に一致する。
- **Trade-offs**: window harnessは同tableの追加entryを実行するが、それらをABI合格範囲として報告しない。失敗時はstability/API evidenceがNo-Goになる。
- **Follow-up**: 全共通entryのABI契約は`sciter-win32-separation`で再検証する。

### Decision: Build Versus Adopt

- **Context**: process supervision、serialization、timeoutに外部crateを追加するか。
- **Selected Approach**: `std::process`、`std::time`、thread/channel、macOS標準commandを採用し、reportは限定schemaのTSVとMarkdownにする。
- **Rationale**: Phase 0 toolにCargo crateやdependency graphを追加せず、監査対象を小さく保つ。
- **Trade-offs**: 汎用serializationやasync runtimeは使わない。schema evolutionはversion fieldで管理する。

## Synthesis Outcomes

- **Generalization**: 78 acceptance IDを`CriterionResult`、8判定領域を`GateResult`へ正規化し、Go/No-Go evaluatorは個別tool実装へ依存しない。
- **Build vs Adopt**: binary metadataは`lipo`、`otool`、`codesign`、`shasum`を採用する。supervisorとreportだけをRustで構築する。
- **Simplification**: product adapter、shared trait、new crate、database、XCTestを追加しない。Phase 0で必要なone-shot toolとimmutable evidenceだけを作る。
- **Dependency Direction**: Model → Manifest and Sciter → Probes, Harness, Supervisor → Decision → Evidence → Main。永続化はEvidence Storeだけが所有する。

## Risks & Mitigations

- 現行macOSでの成功が過去versionへ誤って一般化される — summaryとdecisionへ実行macOS versionと未検証version非保証を明記する。
- popup表示完了を誤判定する — `PopupWindow.isValid`、official close callback、次iterationのinvalid化を必須eventにする。
- child crashでreportが欠落する — runnerがstaging runを先に予約し、parent supervisorが返すcycle recordをEvidence Storeが永続化する。
- malloc diagnosticsがSciter内部を網羅しない — 検出profileと限界をsummaryに記録し、異常時だけGuard Malloc等で追試する。
- `dlclose`がshutdown後もunsafe — runtime handleをprocess lifetimeまで保持する。
- license回答が得られない — `unresolved`をNo-Goとし、技術検証成功で上書きしない。
- generated bindings更新でABI前提が変わる — file hash/version変更をrevalidation triggerにする。

## References

- [Sciter.js SDK 6.0.3.18 commit](https://gitlab.com/sciter-engine/sciter-js-sdk/-/commit/e31ec0f726bdbe5d0402ad647f3b34feef84654e)
- [Sciter C API](https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-x-api.h)
- [Sciter definitions](https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-x-def.h)
- [Sciter main lifecycle](https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/include/sciter-main.cpp)
- [PopupWindow API](https://gitlab.com/sciter-engine/sciter-js-sdk/-/blob/e31ec0f726bdbe5d0402ad647f3b34feef84654e/docs/md/DOM/Element/README.md)
- [Apple malloc diagnostics](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/ManagingMemory/Articles/MallocDebug.html)
- [Apple crash reports](https://developer.apple.com/documentation/xcode/acquiring-crash-reports-and-diagnostic-logs)
- [Clang AddressSanitizer](https://clang.llvm.org/docs/AddressSanitizer.html)

## Design Review Carry-Forward

前回reviewで残った以下のlocal design issueを再生成時に反映し、第2 Design Review GateでPASSした。

1. **Publication modelの型分離**: R1-R7のcriterion、raw artifacts、run contextだけを持つ`PreDecisionEvidence`を先に構築する。Record completenessを確認後にDecision EvaluatorがR8 criterionと`DecisionRecord`を生成し、`FinalEvidenceBundle`をatomic commitする。decisionを含むcandidateからRecord gateを判定する循環を作らない。
2. **Window lifecycleの完了契約**: `SciterWindowExec`のGET STATE interfaceを追加する。`SC_ENGINE_DESTROYED` callbackはcontextへdestroyed flagを記録するだけとし、context解放はcallback復帰後にmain loop ownerが行う。100 transient window完了後にcontroller main windowをcloseし、LOOP returnを確認してからSHUTDOWNする。
3. **Evidence root排他**: run開始時にevidence root単位のexclusive lockを取得し、run commitと`decision.md` publish完了まで保持する。並列runをfail-fastで拒否し、古いrunが新しいcurrent decisionを上書きできないようにする。

### Final Review Resolution

- `CollectedEvidence`はR1-R5と一意な`LicenseValidation`だけを持ち、`RecordValidation`との組合せで`PreDecisionEvidence`になる。Decision Evaluatorが初めてR8とDecisionRecordを生成するため型循環はない。
- `SciterWindowExec`のGET STATE、callback復帰後のcontext解放、controller close、LOOP ITERATION停止、SHUTDOWNの順序をcontract化した。
- evidence root lockはrun IDを所有し、同じIDだけをreserve/commit/publishへ渡す。lock保持中だけcurrent decisionを更新する。
- SDK repository内pathとworkspace配置pathを別fieldへ分離した。
- R6.1-R6.11は`LicenseValidation`がevaluation前に一度だけ生成し、Decision Evaluatorは再解釈しない。

# Implementation Plan

- [ ] 1. Standalone evidence toolkitの基盤を構築する
- [x] 1.1 native arm64向けbuild entryとtest entryを実装する（1-3時間）
  - `run`と`test`の実行modeを受け付け、repository rootから直接native `rustc`を呼び出す。
  - native host architectureとRust compilation prerequisitesだけをbinary実行前に確認し、不足時は製品buildへfallbackせず診断付きで停止する。
  - metadata commandの欠落はBuildEntryで即時停止せず、ArtifactProbeとEvidenceRunnerがenvironment failureとして検出し、unsafe childを起動しないNo-Go evidenceへcommitできるようにする。
  - build outputをtemporary directoryへ隔離し、製品Cargo target、workspace、release artifactを変更しない。
  - hostまたは生成binaryがnative `arm64`でない場合に検証を開始しないことを確認できる。
  - _Requirements: 2.3, 2.4, 7.2, 7.3, 7.4, 7.7, 7.8, 7.9_
  - _Boundary: BuildEntry_

- [x] 1.2 78 criterionと8 gateを表す共有evidence modelを実装する（1-3時間）
  - criterion status、gate status、decision state、cycle event、run identity、typed errorを定義する。
  - 1.1から8.11までの78 IDと、Requirements 1から7の67 source IDを重複なく列挙する。
  - conditional criterionでのみNotApplicableを許可し、missing、duplicate、unknown IDを失敗として扱う。
  - 1から100以外のcycle番号と不正なevent phaseをmodel境界で拒否できる。
  - _Requirements: 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 7.8, 7.9, 7.11, 7.12, 8.1, 8.2, 8.3_
  - _Boundary: Evidence Types_

- [x] 1.3 固定artifact manifestのstrict input contractを実装する（1-3時間）
  - schema version、公式repository、固定commit、SDK内path、workspace内path、SHA-256、engine/API version、同revision header情報を必須入力にする。
  - repositoryを`https://gitlab.com/sciter-engine/sciter-js-sdk`、commitを`e31ec0f726bdbe5d0402ad647f3b34feef84654e`として固定する。
  - runtime path、期待SHA-256、engine version 6.0.3.18、API version 10を固定値として保持する。
  - 全必須値を持つschema version 1の初期artifact manifestをevidence input位置へ作成・配置する。
  - unknown key、duplicate key、missing key、不正なhashまたはversionを明示的なmanifest errorとして確認できる。
  - 配置した初期manifestがstrict parserで成功し、固定期待値を持つArtifactManifestとして観測できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 3.1, 3.2, 4.1, 4.2_
  - _Boundary: ArtifactManifest_
  - _Depends: 1.2_

- [x] 1.4 license evidenceのstrict input contractと初期入力を実装する（1-3時間）
  - redistributionとresigningをPermitted、Prohibited、Unresolvedのいずれかで表す。
  - LICENSE、EULA、permission source、About表記、同梱必須文書を必須入力にし、空値や不明なstatusを拒否する。
  - 固定revisionのLICENSEとSCITER-ENGINE-EULA.mdを識別し、提供元回答未取得の権限項目は推測せずUnresolvedとして表現する。
  - 権威ある公開文書または書面回答を後から入力してもschemaを変えず検証できる。
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6_
  - _Boundary: LicenseEvidence_
  - _Depends: 1.2_

- [ ] 2. Runtime artifactとhost metadataを検証する
- [x] 2.1 (P) runtime identityとMach-O metadata probeを実装する（1-3時間）
  - runtimeをcanonicalizeし、manifestのworkspace pathと一致する場合だけ標準commandを実行する。
  - hash、全architecture、minimum macOS version、外部依存、install name、署名状態をtyped resultとraw stdout/stderrとして返す。
  - 公式repository内pathとworkspace配置pathを別の証拠として扱い、実hashと期待hashの比較結果を生成する。
  - hash不一致、由来未確認、arm64欠落、必須command欠落のいずれでも該当gateがPassにならないことを確認できる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 2.1, 2.2, 2.5, 2.6, 2.7, 2.8, 7.5, 7.6_
  - _Boundary: ArtifactProbe_
  - _Depends: 1.3_

- [x] 2.2 host snapshotとsame-revision header comparisonを実装する（1-3時間）
  - hardware、macOS version、実行process architecture、実行日時を標準commandから収集する。
  - manifestで指定された同revision headersからengine/API version定数を取得し、committed bindingsの定数と比較する。
  - headerが欠落またはrevisionを確認できない場合はnetwork fallbackせず、ABI criterionをNotRunとして返す。
  - raw host情報とheader比較結果が後続のevidence storeへ渡せるartifact bundleとして得られる。
  - _Requirements: 2.3, 2.4, 3.1, 3.2, 4.1, 4.2, 4.7, 7.1, 7.2, 7.3, 7.4_
  - _Boundary: ArtifactProbe_

- [x] 2.3 artifact probeのfailure matrixを自動テストする（1-3時間）
  - hash mismatch、arm64欠落、canonical path mismatch、command failureをfixture出力で再現する。
  - architecture、minOS、dependency、install name、signatureのraw outputが欠落せず返ることを検証する。
  - provenanceまたはhashを確認できないcaseがFailとなり、unsafe childを実行可能な状態にしないことを確認する。
  - _Requirements: 1.5, 1.6, 1.7, 2.1, 2.2, 2.5, 2.6, 2.7, 2.8_
  - _Boundary: ArtifactProbe_

- [ ] 3. Sciter runtimeの限定FFIとAPI/ABI smokeを実装する
- [x] 3.1 (P) absolute-path限定のSciter runtime load境界を実装する（1-3時間）
  - macOS `libSystem`のdynamic loadingだけを使用し、探索path、current directory、環境変数fallbackを使用しない。
  - `SciterAPI` exportを正確なsignatureへ一度だけ変換し、API table pointerのnullを検査する。
  - runtime handleをprocess lifetimeまで保持し、途中で`dlclose`しない。
  - load、export解決、API table取得の各段階を区別したtyped resultと診断が得られる。
  - _Requirements: 3.3, 3.4, 3.5, 3.8_
  - _Boundary: SciterRuntime_
  - _Depends: 1.2, 1.3_

- [x] 3.2 committed bindingsによるversion限定ABI accessを実装する（1-3時間）
  - read-only includeした既存bindings型からAPI version fieldと`SciterVersion` entryだけを取得する。
  - fieldとfunction pointerをnull checkした後に、native arm64 main threadでengine versionを取得する。
  - API 10およびengine 6.0.3.18との比較結果とprocess終了状態を返す。
  - version fieldと`SciterVersion`以外をABI検証済み範囲として報告しないことがresultへ明示される。
  - _Requirements: 3.6, 3.7, 3.9, 4.3, 4.4, 4.5, 4.6, 4.8, 4.9, 4.10_
  - _Boundary: SciterRuntime_

- [x] 3.3 lifecycleで必要なSciter callの安全境界を実装する（1-3時間）
  - application INIT、LOOP ITERATION、STOP、SHUTDOWNとwindow create、callback、HTML load、state操作、debug outputを個別にnull checkする。
  - API callとcallbackをmain threadに限定し、host/debug contextをstable addressで保持する。
  - destroy callbackではflagだけを設定し、callback復帰後のownerだけがcontextを解放できる。
  - lifecycle callの成功がRequirement 4のABI保証範囲を拡張しないことをresultへ明示できる。
  - _Requirements: 4.8, 4.9, 4.10, 5.3, 5.5, 5.8_
  - _Boundary: SciterRuntime_

- [x] 3.4 one-shot API/ABI child modeとnative smoke testを実装する（1-3時間）
  - fixed runtimeのload、SciterAPI解決、non-null table、API version、engine version、bindings経由version取得を固定順序で実行する。
  - failure stage、stdout、stderr、exit statusをsupervisorが解釈できる形で出力する。
  - fixed dylibを用いたnative arm64実行で、API 10とengine 6.0.3.18、限定ABI scopeが観測可能になる。
  - mismatchまたは異常終了時にAPI/ABI gateがPassにならないことを確認する。
  - _Requirements: 2.3, 2.4, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 4.3, 4.4, 4.5, 4.6, 4.8, 4.9, 4.10_
  - _Boundary: LifecycleHarness_
  - _Depends: 2.2, 3.1, 3.2_

- [ ] 4. Popupとwindow lifecycle fixtureを実装する
- [x] 4.1 (P) Sciter固有のpopup fixtureとevent state machineを実装する（1-3時間）
  - 実`menu`要素とanchorを持ち、現行MDLumaと同じ`anchor.popup(menu, ...)`形状だけを使用する。
  - `PopupWindow.isValid == true`の確認後だけshown eventを発行する。
  - 次の`Element.post()` iterationでcloseし、official close callback後の次iterationでinvalidを確認してclosed eventを発行する。
  - 100 cycle完了時だけcontroller windowのcloseを要求し、fixture自身はtimeoutやdecisionを持たない。
  - _Requirements: 5.2, 5.4, 5.6, 5.8, 5.9, 5.10_
  - _Boundary: PopupFixture_
  - _Depends: 1.2_

- [x] 4.2 popup child lifecycle modeを実装する（1-3時間）
  - INIT、controller作成、fixture load、event iteration、controller destroy、loop停止、SHUTDOWNの順序を維持する。
  - Sciter debug callbackのUTF-16 protocol lineを検証し、child stdoutへ転送する。
  - 各cycleでStarted、Shown、Closedが一度ずつ順序通りに出力される。
  - controller破棄後もloopが継続する場合はSTOPを一度だけ送信し、停止を確認してからSHUTDOWNする。
  - _Requirements: 5.1, 5.2, 5.4, 5.6, 5.8, 5.9, 5.10_
  - _Boundary: LifecycleHarness_
  - _Depends: 3.3, 4.1_

- [ ] 4.3 transient window child lifecycle modeを実装する（1-3時間）
  - controller windowを維持しながらtransient windowを逐次create、show、closeする。
  - shown state確認後だけCreatedを、`SC_ENGINE_DESTROYED` callback復帰後だけDestroyedを出力する。
  - contextをcallback外で解放してから次cycleへ進み、100 cycle後にcontrollerを正常終了する。
  - 各cycleでStarted、Created、Destroyedが一度ずつ観測できる。
  - _Requirements: 5.1, 5.3, 5.5, 5.7, 5.8, 5.9, 5.10_
  - _Boundary: LifecycleHarness_
  - _Depends: 3.3_

- [ ] 4.4 popupとwindowの1-cycle lifecycle preflightを実装する（1-3時間）
  - popupでisValid確認、close callback、次iterationのinvalid化が順序通り発生することを検証する。
  - windowでshown stateとdestroy callback後のcontext解放をinstrumented resultで検証する。
  - popupまたはwindowの完了eventが欠けるcaseを成功扱いしない。
  - preflight完了時に各fixtureが実Sciter lifecycleを1 cycle正常完了したことを観測できる。
  - _Requirements: 5.1, 5.4, 5.5, 5.6, 5.7, 5.8_
  - _Boundary: LifecycleHarness_
  - _Depends: 4.2, 4.3_

- [ ] 5. Child process supervisionと診断取得を実装する
- [ ] 5.1 (P) versioned child event protocol parserを実装する（1-3時間）
  - prefix、protocol version、cycle kind、cycle番号、phaseをstrictに解析する。
  - protocol外のstdout/stderrをdiagnosticとして保持し、progress eventへ昇格させない。
  - popupとwindowのphase順序、duplicate completion、1から100以外のcycleをprotocol failureにする。
  - 正しいlineからtyped eventが得られ、不正lineのfailure reasonを観測できる。
  - _Requirements: 5.4, 5.5, 5.8, 5.9, 7.11, 7.12_
  - _Boundary: ProcessSupervisor_
  - _Depends: 1.2_

- [ ] 5.2 bounded stdout/stderr drainとchild outcome収集を実装する（1-3時間）
  - stdoutとstderrをreader threadで同時にdrainし、pipe backpressureによる停止を防ぐ。
  - normal exit、nonzero exit、signalを分離し、exit code、signal、最後の正常eventを保持する。
  - raw outputをbounded bufferとして返し、supervisor自身はfilesystemへ書き込まない。
  - childが異常終了しても収集済みcycle progressとdiagnosticを呼出元が取得できる。
  - _Requirements: 5.8, 5.9, 7.8, 7.11, 7.12_
  - _Boundary: ProcessSupervisor_

- [ ] 5.3 per-cycle deadlineとallocator diagnostic profileを実装する（1-3時間）
  - cycle開始または直前cycle完了から10秒以内にcompletionがない場合、childをkillして必ずwaitする。
  - popupとwindowについてfailed cycleと未完了phaseを記録し、残りcycleをNotRunとして扱う。
  - childへ定められたMalloc diagnostic環境だけを設定し、allocator abortを異常終了として分類する。
  - crash reportはPIDと開始時刻が一致するものだけを候補artifactとして返し、不在を成功根拠にしない。
  - _Requirements: 5.6, 5.7, 5.8, 5.9, 7.7, 7.8, 7.11, 7.12_
  - _Boundary: ProcessSupervisor_

- [ ] 5.4 deterministic childによるsupervisor test matrixを実装する（1-3時間）
  - success、nonzero exit、signal、短縮timeout、malformed protocol、duplicate eventを再現する。
  - timeout後のkill/wait、failed cycle、exit/signal、stderr保持を検証する。
  - reader threadが大量diagnostic出力をdrainしてもhangしないことを確認する。
  - 各failure caseがPassではなく明確なSupervisionOutcomeとして観測できる。
  - _Requirements: 5.6, 5.7, 5.8, 5.9, 7.8, 7.11, 7.12_
  - _Boundary: ProcessSupervisor_

- [ ] 6. Immutable evidence storeとrecord validationを実装する
- [ ] 6.1 (P) evidence root lockとrun reservationを実装する（1-3時間）
  - UTC timestampと短いnonceからrun IDを生成し、exclusive root lockへPID、開始時刻、run IDを記録する。
  - 既存lock、run ID collision、既存run directoryをfail-fastで拒否し、自動削除や上書きを行わない。
  - lockが所有するrun IDだけで同一filesystem上のstaging directoryを予約する。
  - 並行writerが一つに限定され、既存runが変更されないことをfilesystem上で確認できる。
  - _Requirements: 7.10, 8.2, 8.3, 8.7, 8.8_
  - _Boundary: EvidenceStore_
  - _Depends: 1.2_

- [ ] 6.2 raw artifacts、events、summaryのserializerを実装する（1-3時間）
  - metadata、API、popup、window、crashのnamed artifactsをrun内relative pathへ分類する。
  - versioned events TSVでtab/newlineを拒否し、全cycle eventをlosslessに保存する。
  - summaryへ日時、hardware、macOS version、process architecture、runtime path/hash、手順、終了状態、criterion/gate結果を出力する。
  - allocator診断の検出範囲と、未実行macOS versionを保証しない制限をsummaryへ明示する。
  - _Requirements: 2.1, 2.5, 2.6, 2.7, 2.8, 5.4, 5.5, 5.9, 5.10, 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.11, 7.12_
  - _Boundary: EvidenceStore_

- [ ] 6.3 record completeness validationとatomic run commitを実装する（1-3時間）
  - collected evidenceのrequired context、raw artifact、criterion resultからRequirements 7のrecord gateを一度だけ生成する。
  - 全artifactとsummaryをstagingへ書いて検証した後、atomic renameでfinal runを公開する。
  - writeまたはrename失敗時はGoを返さず、current decisionを更新しない。
  - commit済みrunがappend-onlyとなり、同じrun IDへの再commitが失敗することを確認できる。
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.10, 7.11, 7.12_
  - _Boundary: EvidenceStore_

- [ ] 6.4 evidence persistenceのfailureとimmutabilityを自動テストする（1-3時間）
  - lock競合、stale lock非自動削除、run collision、invalid TSV、partial writeを検証する。
  - successful commit後のrun内容を保存し、再実行やdecision publishでbyte-for-byte変化しないことを確認する。
  - atomic commit失敗時に未完成runまたは誤ったGo表示が公開されないことを確認する。
  - _Requirements: 7.8, 7.9, 7.10, 7.11, 7.12, 8.2, 8.3_
  - _Boundary: EvidenceStore_

- [ ] 7. License gateとGo/No-Go decisionを実装する
- [ ] 7.1 (P) authoritative license evidence validationを実装する（1-3時間）
  - LICENSE、EULA、About text、同梱文書、再配布、再署名の各criterionを一度だけ評価する。
  - Permittedでもsource、About text、必須文書が空なら該当criterionをUnsatisfiedにする。
  - ProhibitedまたはUnresolvedのredistribution/resigningをLicense gateのFailへ変換する。
  - 入力からRequirements 6の11 criterionと一つのLicense gateが重複なく得られる。
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9, 6.10, 6.11_
  - _Boundary: LicenseEvidence_
  - _Depends: 1.4_

- [ ] 7.2 source criterionとgateのcompleteness evaluatorを実装する（1-3時間）
  - Requirements 1から7の67 criterionと8 gateが一件ずつ存在することを検証する。
  - Unsatisfied、NotRun、missing、duplicate、unknown criterion/gateが一件でもあればNo-Goにする。
  - conditional triggerが成立するcriterionをNotApplicableで通過させない。
  - 全source criterionとgateが完全かつPassの場合だけGo候補が得られる。
  - _Requirements: 8.1, 8.2, 8.3, 8.4_
  - _Boundary: DecisionEvaluator_
  - _Depends: 1.2, 7.1_

- [ ] 7.3 downstream stateとR8 criterion生成を実装する（1-3時間）
  - No-Goまたは未確定ではplatform-contract-extractionとsciter-win32-separationをBlockedにする。
  - Goではplatform-contract-extractionのruntime evidence依存だけをFulfilledにし、sciter-win32-separationはPhase 1待ちとしてBlockedを維持する。
  - No-Go理由と6.0.4.8同一revision更新方針を記録するが、更新処理自体は実装しない。
  - decisionからRequirements 8の11 criterionを生成し、最終78 ID集合を完成させる。
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  - _Boundary: DecisionEvaluator_

- [ ] 7.4 licenseとdecisionのtable-driven testを実装する（1-3時間）
  - redistribution/resigningのPermitted、Prohibited、Unresolvedと根拠欠落を網羅する。
  - gate Fail、NotRun、criterion欠落、duplicate、unknown、全Passを入力してdecisionを検証する。
  - Go、No-Goそれぞれのdependency state、reason、fallback、R8 criterion集合を確認する。
  - 完了runへPendingが残らず、No-Goでも全78 criterionが監査可能な状態になることを確認する。
  - _Requirements: 6.7, 6.8, 6.9, 6.10, 6.11, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  - _Boundary: DecisionEvaluator_

- [ ] 8. Evidence pipelineを統合する
- [ ] 8.1 CLI mode dispatchとfixed invocation contractを統合する（1-3時間）
  - operator用run/test modeと内部ApiAbi、PopupCycles、WindowCycles child modeを分離する。
  - repository root、manifest、license input、runtime、fixtureを固定したlocal pathから解決する。
  - child modeを通常operator invocationから誤って直接decision publishできないようにする。
  - `run.sh run`と`run.sh test`から同じnative binary contractが再現できる。
  - _Requirements: 3.3, 5.1, 7.5, 7.7_
  - _Boundary: BuildEntry, EvidenceRunner_
  - _Depends: 1.1, 3.4, 4.2, 4.3_

- [ ] 8.2 fail-safe Evidence Runner orchestrationを統合する（1-3時間）
  - input parse、lock、run reservation、artifact/header probe、各child、license validation、record validation、decisionの固定順序を実行する。
  - artifactまたはheader prerequisite失敗時はunsafe childを起動せず、残りをNotRunとしてNo-Go evidenceへ含める。
  - individual child failure後も残りの収集可能な証拠を保持し、finalizeを試行する。
  - success、partial failure、prerequisite failureの各runで最終bundle候補が一つだけ生成される。
  - _Requirements: 1.5, 1.6, 1.7, 2.2, 2.4, 3.8, 3.9, 4.7, 4.8, 4.9, 5.8, 5.9, 7.9, 7.11, 7.12, 8.3, 8.4_
  - _Boundary: EvidenceRunner_
  - _Depends: 2.3, 3.4, 4.4, 5.4, 6.3, 7.3_

- [ ] 8.3 atomic current decision publicationを統合する（1-3時間）
  - committed runを参照するcurrent decisionをroot lock保持中だけatomic replaceする。
  - run commit後のdecision publish失敗ではcommandをnonzero終了し、run summaryをauthoritative recordとして保持する。
  - Goはrun commitとdecision publicationが成功した後だけoperator outputへ表示する。
  - latest decisionが対応するimmutable run ID、Go/No-Go理由、downstream stateを示す。
  - _Requirements: 7.8, 7.9, 7.10, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  - _Boundary: EvidenceRunner, EvidenceStore_
  - _Depends: 6.3, 7.3, 8.2_

- [ ] 8.4 pipeline integration failure pathsを自動テストする（1-3時間）
  - bad manifest、hash mismatch、API child failure、popup timeout、window signal、license unresolved、persistence failureを注入する。
  - 各caseでunsafe後続処理の抑止、診断保持、No-Goまたはfatal exit、current decision保護を確認する。
  - 製品crate、製品source、Windows runtime path、Cargo metadataへ依存せずtest binaryだけで実行できる。
  - _Requirements: 1.6, 1.7, 2.2, 2.4, 3.8, 3.9, 4.7, 4.8, 4.9, 5.8, 5.9, 6.7, 6.8, 6.9, 6.10, 6.11, 7.11, 7.12, 8.3, 8.4_
  - _Boundary: EvidenceRunner_
  - _Depends: 8.3_

- [ ] 9. Recorded Apple Silicon macOS上でend-to-end evidenceを検証する
- [ ] 9.1 identityからdecision publicationまでを単一invocationでE2E実行する（1-3時間）
  - recorded current Apple Silicon macOS上で一度だけrunnerを起動し、固定runtimeのidentity、platform metadata、API/限定ABI smokeを収集する。
  - 同じinvocation内でpopupとwindowを各100 cycle実行し、各cycleの完了event、10秒deadline、exit/signal、allocatorおよびcrash diagnosticを収集する。
  - 固定revisionのLICENSE、EULA、必須配布文書、About表記、再配布・再署名根拠を取り込み、提供元根拠が未取得ならUnresolvedを維持してNo-Goにする。
  - Requirements 1から7と8 gateを一つのdecisionへ集約し、実行macOS versionへ限定したsummary、全No-Go理由、downstream state、6.0.4.8 fallback方針を生成する。
  - identity、platform、API、ABI、popup、window、license、decisionの全artifactが一つのimmutable runとして一度だけatomic commitされ、そのcommitted runを参照するcurrent decisionがpublishされる。
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8, 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9, 4.1, 4.2, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8, 4.9, 4.10, 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9, 5.10, 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 6.7, 6.8, 6.9, 6.10, 6.11, 7.1, 7.2, 7.3, 7.4, 7.7, 7.8, 7.9, 7.11, 7.12, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  - _Boundary: ArtifactManifest, ArtifactProbe, SciterRuntime, LifecycleHarness, PopupFixture, ProcessSupervisor, LicenseEvidence, EvidenceStore, DecisionEvaluator, EvidenceRunner_
  - _Depends: 8.4_

- [ ] 9.2 rerun、concurrency、spec boundaryの最終回帰を検証する（1-3時間）
  - 同じruntimeで再実行し、新しいrun directoryが追加され、以前のrunがbyte-for-byte不変であることを確認する。
  - 並行runnerを開始し、後発runがroot lockで拒否され、current decisionを更新しないことを確認する。
  - 全runで日時、hardware、macOS、architecture、runtime path/hash、手順、終了状態、status、failure stage、診断が必要に応じて記録されることを確認する。
  - 製品`src/`、製品Cargo metadata、Windows runtime経路、packaging、署名、notarization、6.0.4.8 artifactが変更されていないことを確認する。
  - 最終summaryに78 criterionが重複なく存在し、current decisionがcommitted runだけを参照することを確認する。
  - _Requirements: 5.10, 7.1, 7.2, 7.3, 7.4, 7.5, 7.6, 7.7, 7.8, 7.9, 7.10, 7.11, 7.12, 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7, 8.8, 8.9, 8.10, 8.11_
  - _Boundary: BuildEntry, EvidenceStore, DecisionEvaluator, EvidenceRunner_
  - _Depends: 9.1_

## Implementation Notes

- Task 1.1: Rust stableは`mise`管理下にあり、task-local verificationは`mise exec rust@stable -- ...`で実行する。
- Task 1.2: Evidence Typesは静的catalogと型検証だけを所有し、67 criterion/8 gateのcompleteness判定はTask 7.2のDecisionEvaluatorに残す。
- Task 1.3: 固定manifestはtyped値だけでなく全12 fieldのraw canonical textも一致させ、正規化で同値になる入力を拒否する。
- Task 1.4: 未解決permissionでもsourceは固定revision EULAを明示し、statusだけを`unresolved`として保持する。
- Task 2.1: system metadata commandは固定absolute pathだけを使用し、Platform gateはTask 2.2完了まで`NotRun`に保つ。
- Task 2.2: authoritative headersが欠落またはrevision不明ならnetwork fallbackせず、header/ABI evidenceを`NotRun`にする。
- Task 2.3: 未収集criterionが残っていても既知の`Unsatisfied`はgateを`Fail`にできるが、不完全な入力から`Pass`は生成しない。
- Task 3.1: Sciter runtimeはmanifestと一致するcanonical absolute pathだけを`RTLD_NOW | RTLD_LOCAL`で開き、handleをprocess lifetimeまで保持する。
- Task 3.2: committed bindingsのABI claimはAPI `version` fieldと`SciterVersion` entryだけに限定し、selector順序`0,1,2,3`を固定する。
- Task 3.3: registered callback contextは`Pin<Box<_>>`で固定し、destroy callback復帰後までownerが解放しない。
- Task 3.4: childはunsafe operation直前のstageをflushし、exit未観測時はgate Passではなく`success_candidate`だけを出力する。
- Task 4.1: Nodeはpure reducerだけを検証し、Sciter API shapeはasset contractに限定する。実runtime lifecycleはTask 4.2で検証する。
- Task 4.2: pinned wrapperに合わせてSTOPだけがraw zeroを要求し、INIT/SHUTDOWNは正常復帰を受理する。primaryとshutdown failureは両方保持する。

# Requirements Document

## Introduction

本仕様は、MDLumaの共有viewerロジックからWindows固有サービスの選択と解釈を分離し、後続のmacOS adapterを追加できるplatform contractを確立する。対象はnative dialog、外部URL、設定・debug log保存先、既定document opener、window操作、および起動時のplatform選択である。Windows 10/11の既存viewer体験、保存形式、失敗時の状態を変更せず、Sciter runtime分離やmacOS adapter実装は後続仕様へ委ねる。

## Boundary Context

- **In scope**: OS service contract、既存Windows serviceの適合、file/font dialog、window操作とcascade/geometry利用、外部URL opening、application-data/log directory解決、default document opening、起動動作、およびWindows viewer動作の回帰確認。
- **Out of scope**: macOS service実装、Sciter dynamic loader分離、WndProc・Win32 message分離、native-frame UI、runtime version更新、`.app` packaging、署名、notarization、設定形式の変更、新しいviewer機能。
- **Adjacent expectations**: 本仕様の完了後に`sciter-win32-separation`、続いて`macos-sciter-host-smoke`へ進む。本仕様はsuspend中の`macos-sciter-runtime-evidence`の完了を開始条件としない。

## Requirements

### Requirement 1: Platform Service利用時のViewer一貫性
**Objective:** As a macOS移植開発者, I want OS serviceを変更しても共有viewerの動作が一貫していてほしい, so that 新しいOS対応によってviewer機能を変えずに済む

#### Acceptance Criteria
1. When MDLumaがfile選択、font選択、window操作、外部URL opening、directory解決、またはdocument openingを要求する, the MDLuma shall 実行中のplatformが提供するserviceへ要求を渡す
2. When platform serviceが成功する, the MDLuma shall 対応するviewer操作を一度だけ完了する
3. If platform serviceが操作のcancelを返す, then the MDLuma shall 現在のdocument、設定、およびviewer状態を維持する
4. If platform serviceが失敗する, then the MDLuma shall applicationを異常終了せず、失敗前のdocumentを失わない
5. The MDLuma shall platform serviceの変更によってMarkdown変換結果、HTML表示内容、またはviewer状態遷移を変更しない

### Requirement 2: Native File・Font選択の既存動作
**Objective:** As a Windowsユーザー, I want fileとfontの選択操作が構造変更後も同じように動作してほしい, so that 既存のviewer操作を継続して利用できる

#### Acceptance Criteria
1. When WindowsユーザーがMarkdown fileを開く操作を行う, the MDLuma shall native file選択画面でMarkdown fileを選択できるようにする
2. When Windowsユーザーが外部editorの選択操作を行う, the MDLuma shall native file選択画面でWindows実行fileを選択できるようにする
3. If ユーザーがいずれかのfile選択をcancelする, then the MDLuma shall 現在のdocument、設定、および表示状態を変更しない
4. If file選択serviceが失敗する, then the MDLuma shall 選択前のdocumentと設定を維持し、platformから受け取った失敗原因を操作の呼び出し元へ返す
5. When Windowsユーザーが本文fontの選択操作を行う, the MDLuma shall 現在のfont設定を初期値とするnative font選択画面を表示する
6. When ユーザーがfontを確定する, the MDLuma shall 選択されたfont familyとpoint sizeを既存の本文font設定へ反映する
7. If ユーザーがfont選択をcancelする, then the MDLuma shall 現在の本文font設定と表示状態を維持する
8. If font選択serviceが失敗する, then the MDLuma shall 現在の本文font設定と表示状態を維持し、debug buildの診断logへ失敗を記録する

### Requirement 3: Native Window操作とWindow情報
**Objective:** As a Windowsユーザー, I want window操作と配置が構造変更後も維持されてほしい, so that viewerを従来どおり管理できる

#### Acceptance Criteria
1. When ユーザーが最小化操作を選択する, the MDLuma shall 現在のviewer windowを最小化する
2. When ユーザーが最大化または元に戻す操作を選択する, the MDLuma shall viewer windowを対応する状態へ切り替える
3. When ユーザーが閉じる操作を選択する, the MDLuma shall 現在のviewer windowへ正常な終了を要求する
4. When viewer windowの最大化状態が変化する, the MDLuma shall Windows上の既存corner表示を維持する
5. If native window参照が無効または利用不能である, then the MDLuma shall applicationを異常終了せず、失敗したwindow操作と原因を操作の呼び出し元へ返す
6. When 複数documentのviewer windowを起動する, the MDLuma shall 利用可能な親window位置を用いた既存のcascade配置を維持する
7. If Windowsで親window位置を取得できない, then the MDLuma shall `(0, 0)`をcascade計算の基準として子viewerを起動する
8. When viewerのevent loopが正常に終了する, the MDLuma shall 利用可能なwindow geometryを既存設定へ保存する

### Requirement 4: 外部URLのOpening
**Objective:** As a Markdown読者, I want 外部linkを既定browserで開きたい, so that viewerから関連するweb情報へ移動できる

#### Acceptance Criteria
1. When ユーザーが許可された外部`http`または`https` linkを選択する, the MDLuma shall Windowsの既定browserへそのURLのopeningを要求する
2. While 外部URL openingを処理する, the MDLuma shall 現在のdocumentとviewer状態を維持する
3. If OSが外部URLを開けない, then the MDLuma shall applicationを異常終了せず、現在のdocument表示を維持する

### Requirement 5: Settings・Debug Log Directory解決
**Objective:** As a Windowsユーザー, I want 設定とdebug logが従来の場所へ保存されてほしい, so that 構造変更後も既存設定と診断情報を継続利用できる

#### Acceptance Criteria
1. When Windowsのapplication-data directoryを解決できる, the MDLuma shall settings fileを`%LOCALAPPDATA%\MDLuma\settings.json`で読み書きする
2. When Windowsのapplication-data directoryを解決できる, the MDLuma shall debug buildのlogを`%LOCALAPPDATA%\MDLuma\logs\`配下へ書き込む
3. If application-data directoryを解決できない, then the MDLuma shall settingsとdebug logに既存のtemporary directory fallbackを使用する
4. If settings fileを読み込めないまたは解釈できない, then the MDLuma shall applicationを異常終了せずdefault settingsでviewerを開始する
5. If ユーザー操作に伴うsettings保存が失敗する, then the MDLuma shall viewerを実行中のまま現在のdocumentを保持し、settings保存失敗をwindow内に表示する
6. If debug log directoryまたはlog fileを作成できない, then the MDLuma shall applicationを異常終了せず、debug buildでは診断をstandard errorへ出力する
7. While release buildを実行している, the MDLuma shall debug log出力を有効にしない
8. The MDLuma shall 既存settings fileの形式と既存の読み書き結果を変更しない

### Requirement 6: Default Document Openingと選択済みEditor
**Objective:** As a Windowsユーザー, I want 表示中のMarkdownを設定済みまたは既定のeditorで開きたい, so that 必要なときだけ外部applicationで編集へ移れる

#### Acceptance Criteria
1. While documentが読み込まれていない, the MDLuma shall 外部editorで開く操作を利用不可として扱う
2. When documentが読み込まれ、外部editorが設定されている状態でユーザーが外部editorで開く操作を行う, the MDLuma shall 設定されたeditorへ現在のdocument pathを渡して起動する
3. When documentが読み込まれ、外部editorが設定されていない状態でWindowsユーザーが外部editorで開く操作を行う, the MDLuma shall `notepad.exe`へ現在のdocument pathを渡して起動する
4. When 外部editorの起動に成功する, the MDLuma shall 現在のviewer windowへ終了を要求する
5. If 外部editorの起動に失敗する, then the MDLuma shall viewerを終了せず、ユーザーが失敗を認識できる状態を表示する
6. The MDLuma shall 外部editor設定画面をdocumentの読み込み状態にかかわらず利用可能にする

### Requirement 7: Startup動作の維持
**Objective:** As a MDLuma保守者, I want platform対応の準備後もstartup動作が維持されてほしい, so that Windowsユーザーの起動方法を変更せずに済む

#### Acceptance Criteria
1. When Windows版MDLumaを起動する, the MDLuma shall file、font、window、URL、directory、およびdocument openingにWindowsの既存動作を提供する
2. When 引数なしでMDLumaを起動する, the MDLuma shall document未読み込みのviewerを表示する
3. When 一つのdocument pathでMDLumaを起動する, the MDLuma shall そのdocumentを読み込むviewerを開始する
4. When 2個以上のdocument pathでMDLumaを起動する, the MDLuma shall 入力順を維持して最大10個の子viewerを起動する
5. If runtimeまたは必須assetの事前検証が失敗する, then the MDLuma shall viewerを表示せず、ユーザー向けerrorと原因を追跡できる診断情報を返す

### Requirement 8: 移行範囲とWindows回帰
**Objective:** As a MDLumaリリース担当者, I want platform境界の抽出が既存viewer機能を変更しないことを確認したい, so that macOS移植準備によるWindows回帰を防げる

#### Acceptance Criteria
1. While 本仕様の変更をWindows 10またはWindows 11で実行する, the MDLuma shall 既存のfile open、font、theme、search、text selection/copy、drag-and-drop、recent files、およびwindow操作を維持する
2. The MDLuma shall 既存のviewer state transition、error復帰、および単一document表示動作を維持する
3. When ユーザーがWindows custom title barのcaption領域をdragする, the MDLuma shall viewer windowを従来どおり移動する
4. When document表示を置き換えた後にユーザーがMarkdown fileをdropする, the MDLuma shall dropされたfileを従来どおり開く

# Research & Design Decisions

## Summary
- **Feature**: `command-line-file-open`
- **Discovery Scope**: Extension
- **Key Findings**:
  - 既存の `AppController`、`DocumentLoader`、`ViewerState`、`ViewerError` で、単一ファイルの読込・エラー表示・単一ドキュメント制約はすでに表現できる。
  - 欠けているのは、起動引数の分類、未対応 `--` 引数の非致命 `stderr` 通知、複数ファイル時の子プロセス起動である。
  - Sciter の初回表示後に再ロード回数を増やすと表示順序リスクが上がるため、起動ファイルは初回 `show_initial()` 前に状態化する方が既存の安定化方針と整合する。

## Research Log

### 起動経路と責務境界
- **Context**: 起動引数処理をどこへ追加しても既存責務を壊さないかを確認する必要があった。
- **Sources Consulted**:
  - `src/main.rs`
  - `src/lib.rs`
  - `src/app.rs`
  - `.kiro/steering/structure.md`
- **Findings**:
  - `main.rs` は `mdluma::run()` 呼び出しと致命的起動失敗の `stderr` 報告に限定された薄い入口である。
  - `lib.rs` は runtime 検証、UI 構築、`AppController` 構築を束ねる composition root である。
  - `AppController` は UI イベント経由のファイルオープンを制御し、内部 state を `ViewerState` で保持する。
- **Implications**:
  - 起動引数の分類は新規モジュールで分離し、起動計画の実行は `lib.rs` に置く。
  - `main.rs` は薄いまま維持し、複雑な CLI 解釈を吸い込まない。

### 既存の単一ファイル表示能力
- **Context**: 新機能が新しい表示モデルを必要とするかを見極める必要があった。
- **Sources Consulted**:
  - `src/app.rs`
  - `src/document.rs`
  - `src/errors.rs`
  - `src/html_shell.rs`
- **Findings**:
  - `FileDocumentLoader` は `Path` から UTF-8 Markdown を読み込み、`FileRead` と `InvalidEncoding` を返せる。
  - `ViewerState` は `NoDocument` / `DocumentLoaded` / `ErrorVisible` の 3 状態で、`document_count()` は常に 0 または 1 になる。
  - `html_shell` はファイル未読込時の空状態と、読込失敗時のエラー表示を既存 UI に埋め込める。
- **Implications**:
  - 新機能は新しいドキュメントモデルを追加しない。
  - 1 インスタンス 1 ファイル要件は、既存 `ViewerState` の不変条件をそのまま利用する。

### Sciter 初回表示と起動ファイルの扱い
- **Context**: 起動時にファイルを開く際、初回表示後の再ロードが必要かどうかを判断する必要があった。
- **Sources Consulted**:
  - `design/sciter-startup-display-investigation.md`
  - `src/sciter/window.rs`
  - `src/lib.rs` の起動順テスト
- **Findings**:
  - 既存実装では `bind -> show_initial -> event_loop` の順を保っている。
  - 表示済みウィンドウに対する HTML 再ロードは Windows で deferred load を必要とし、Sciter コールバック内の同期再ロードは危険である。
  - 初回 `show_initial()` 前に state を確定できれば、起動時ファイル表示は 1 回の初期 HTML 読込に収まる。
- **Implications**:
  - 起動ファイルは `AppController` の起動前 state 準備として処理する。
  - 空画面起動後に `show_document()` を追加で呼ぶ設計は採らない。

### 依存と build vs adopt
- **Context**: CLI 解析や子プロセス起動のために新規依存が必要か確認する必要があった。
- **Sources Consulted**:
  - `Cargo.toml`
  - `.kiro/steering/product.md`
  - `.kiro/steering/tech.md`
  - Rust standard library documentation: `std::env::args_os`, `std::process::Command`
- **Findings**:
  - 現在の依存は `comrak` のみで、CLI 解析ライブラリは導入されていない。
  - 要件が必要とする引数規則は単純であり、`args_os()` と `Command` で十分表現できる。
  - プロダクト方針は軽量性と最小責務を重視している。
- **Implications**:
  - `clap` などの外部 CLI ライブラリは採用しない。
  - 子プロセス起動は標準ライブラリベースで設計する。

## Architecture Pattern Evaluation

| Option | Description | Strengths | Risks / Limitations | Notes |
|--------|-------------|-----------|---------------------|-------|
| 既存 `lib.rs` へ直書き | 引数解析、起動分岐、spawn を `lib.rs` に直接追加 | 最小ファイル数 | 起動組立と引数分類が混在しやすい | 却下 |
| 小さな引数分類モジュール + `lib.rs` オーケストレーション | 引数分類だけ分離し、実行は既存 startup root に残す | 責務が明確、テストしやすい、過剰抽象を避ける | `lib.rs` に起動計画実行の責務は残る | 採用 |
| 外部 CLI ライブラリ導入 | 引数分類をライブラリへ委譲 | 拡張余地が大きい | 依存増加、起動コストと複雑性が不要に上がる | 却下 |

## Design Decisions

### Decision: 起動引数は専用の小モジュールで分類する
- **Context**: 要件 1, 2, 3, 4 はすべて「起動時に受け取った OS 引数の分類」に集約される。
- **Alternatives Considered**:
  1. `main.rs` / `lib.rs` へ分類ロジックを直接埋め込む
  2. `src/startup_args.rs` へ純粋な分類ロジックを分離する
- **Selected Approach**: `src/startup_args.rs` に `StartupLaunchPlan`、`LaunchAction`、`StartupNotice` を定義し、`OsString` ベースで分類する。
- **Rationale**: 引数分類を副作用なしでテストでき、`lib.rs` の責務を「計画の実行」に限定できる。
- **Trade-offs**: ファイルは 1 つ増えるが、spawn と UI 起動の分岐を読みやすく保てる。
- **Follow-up**: 11 件超過の切り捨てと `--` 通知が plan レベルで完結していることを単体テストで固定する。

### Decision: 起動ファイルは初回表示前に `ViewerState` へ解決する
- **Context**: 起動時ファイル表示を既存 Sciter 安定化方針と両立する必要がある。
- **Alternatives Considered**:
  1. 空画面を出してから `show_document()` で開く
  2. `AppController` の起動前準備として state を確定する
- **Selected Approach**: `AppController` に起動前パス準備の小さな入口を追加し、初回 `show_initial()` が最終 state を描画する。
- **Rationale**: 追加の HTML 再ロードを避け、単一ファイル成功時も失敗時も 1 回の初期表示で表現できる。
- **Trade-offs**: `AppController` に startup 専用の入口が 1 つ増える。
- **Follow-up**: 初回表示時に `document_html` ではなく `initial_html` に内容が出ることを統合テストで固定する。

### Decision: 複数ファイルは親プロセスでファンアウトし、子プロセスは 1 ファイルだけ受け取る
- **Context**: 既存 `ViewerState` と UI は単一ドキュメント前提である。
- **Alternatives Considered**:
  1. 1 プロセス内で複数ウィンドウを生成する
  2. 親プロセスが 1 ファイルずつ子プロセスを起動する
- **Selected Approach**: `LaunchAction::SpawnChildren` を実行し、各子プロセスへ 1 件の `PathBuf` だけを渡して即座に親処理を終える。
- **Rationale**: 単一ドキュメント前提を維持でき、ファイルごとの読込失敗も子プロセスごとの UI エラーとして自然に分離できる。
- **Trade-offs**: 親プロセスの spawn 失敗は一部子起動後に表面化する可能性がある。
- **Follow-up**: spawn の順序、親が viewer を起動しない条件、部分成功時の扱いを統合テストで固定する。

### Decision: `stderr` は CLI ユーザー向け通知に限定して例外的に使う
- **Context**: 技術方針では `stderr` を通常診断ログに広げないが、本要件では未対応 `--` 引数を標準エラーへ出力する必要がある。
- **Alternatives Considered**:
  1. `debug_log!` のみで通知する
  2. `stderr` をユーザー向け CLI 通知として限定利用する
- **Selected Approach**: unsupported `--` 通知と致命的 startup failure だけを `stderr` へ出し、診断ログ用途には広げない。
- **Rationale**: 要件を満たしつつ、技術方針の「診断ログは `debug_log!`」も保てる。
- **Trade-offs**: `stderr` の用途が 1 つ増えるため、将来の CLI 拡張でも同じ制約を守る必要がある。
- **Follow-up**: 通知メッセージ書式を英語固定にし、1 引数 1 行であることをテストへ落とす。

## Risks & Mitigations
- Sciter 初回表示後の再ロードが増えると表示不具合リスクが上がる — 起動ファイルは初回表示前に state 化し、起動成功時の追加 `show_document()` を避ける。
- 起動責務が `main.rs` / `lib.rs` / `app.rs` で曖昧になる — 引数分類は `startup_args.rs`、計画実行は `lib.rs`、状態解決は `app.rs` に固定する。
- 複数ファイル時の部分的 spawn 失敗が扱いづらい — すでに起動した子は継続し、親は `StartupError` で失敗を返す契約を明記する。
- `--` で始まる実在ファイル名が開けない — 要件上の意図的制限として design.md の Out of Boundary へ明記する。

## References
- `src/main.rs` — バイナリエントリポイントと致命的 startup failure の `stderr` 経路
- `src/lib.rs` — runtime 検証、controller 構築、起動順テスト
- `src/app.rs` — `AppController`、`ViewerState`、既存 open フロー
- `src/document.rs` — `FileDocumentLoader` と UTF-8 / file read エラー境界
- `src/errors.rs` — `StartupError` / `ViewerError` のユーザー向け文言と診断文言
- `design/sciter-startup-display-investigation.md` — 起動直後表示と再ロード順序の制約
- [Rust std::env::args_os](https://doc.rust-lang.org/std/env/fn.args_os.html) — OS 依存文字列を保持した引数取得
- [Rust std::process::Command](https://doc.rust-lang.org/std/process/struct.Command.html) — 子プロセス起動 API

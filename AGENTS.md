# Agent Guidelines

- Before making design or implementation decisions, read `design/initialdesign.md` and follow its direction.
- Keep MDLuma lightweight, fast to start, memory-conscious, simple, and minimal.
- Implement only viewer-related functionality unless external application integration is clearly useful.
- Target Windows 11/10 first, while keeping the design portable for future macOS and Linux support.
- Use Rust as the primary language. The default build target is `x86_64-pc-windows-msvc`.
- Render Markdown by converting it to HTML first. Use Comrak with GFM support for Markdown conversion.
- Use Sciter.js SDK for HTML rendering when implementing the application UI. Do not confuse it with older Sciter.TIS.
- For any Sciter-related work, including HTML, CSS, and JavaScript authoring, do NOT code by WebView/browser assumptions. Sciter differs significantly in event flow, scripting, DOM APIs, layout, styling, and window behaviors. Always verify against Sciter's own references before implementing or editing:
  - **Documentation**: `vendor/sciter-js-sdk-main/docs/md/` — Sciter's official docs on APIs, behaviors, and specifics.
  - **Samples**: `vendor/sciter-js-sdk-main/` — contains Sciter sample implementations. Refer to them for usage patterns.
  - **C/C++ headers**: `vendor/sciter-js-sdk-main/include/` — contains header files (`*.h`) where API definitions, constants, and structs are the authoritative source of truth.
- Before introducing or changing Sciter UI code, inspect the relevant docs and samples first, then implement to match Sciter's actual behavior.
- When looking up Sciter information, prefer this order: existing project design notes, Sciter SDK docs/samples/headers via `ccc search`, official Sciter documentation pages with known valid URLs.
- Avoid speculative WebFetch requests to guessed Sciter forum or documentation URLs. If an official page is not known to exist, use `ccc search` or inspect the SDK headers instead.
- For UI work, consult `design/initial-image.png`, `design/sample-index.html`, and assets under `assets/` as needed.
- Develop incrementally. Prefer small, focused changes over broad rewrites.
- If visual studio build tools such as `nmake.exe` and `link.exe` are unavailable in `PATH`, run `Enter-Vs` function to make PATH include the tools.
- If you need to use grep, use rg (ripgrep) instead.

## Code search policy
Before using grep, ripgrep, find, or broad file reads to understand the codebase,
use cocoindex-code via the ccc skill.

Use ccc for:
- finding implementations by meaning
- locating related symbols
- understanding call paths
- searching across unfamiliar parts of the repository
- finding similar code or duplicates

Not use ccc for searching under vender directory. no index for vender/.

Prefer:
- `ccc search "<natural language query>"`
- `ccc search "<symbol or behavior>"`
- `ccc status` when unsure whether the index is ready

Use `rg` only after ccc has identified likely files, or when doing exact string matching.

## RTK
`rtk` is installed. For these shell commands, use `rtk` by simply prefixing the original command and keeping all arguments unchanged: `git`, `gh`, `aws`, `docker`, `rg`, `cargo`, `npm`, `npx`, `pip`, `go`.

If `rtk` itself causes the command to fail or behave incorrectly, rerun the same command without `rtk`.

Skip `rtk` only when raw output is explicitly needed or the user asks for the original command.

## Language
Use English for source code comments and Git commit messages.

# Agentic SDLC and Spec-Driven Development

Kiro-style Spec-Driven Development on an agentic SDLC

## Project Memory
Project memory keeps persistent guidance (steering, specs notes, component docs) so OpenCode honors your standards each run. Treat it as the long-lived source of truth for patterns, conventions, and decisions.

- Use `.kiro/steering/` for project-wide policies: architecture principles, naming schemes, security constraints, tech stack decisions, api standards, etc.
- Use local `AGENTS.md` files for feature or library context (e.g. `src/lib/payments/AGENTS.md`): describe domain assumptions, API contracts, or testing conventions specific to that folder. OpenCode auto-loads these when working in the matching path.
- Specs notes stay with each spec (under `.kiro/specs/`) to guide specification-level workflows.

## Project Context

### Paths
- Steering: `.kiro/steering/`
- Specs: `.kiro/specs/`

### Steering vs Specification

**Steering** (`.kiro/steering/`) - Guide AI with project-wide rules and context
**Specs** (`.kiro/specs/`) - Formalize development process for individual features

### Active Specifications
- Check `.kiro/specs/` for active specifications
- Use `/kiro-spec-status [feature-name]` to check progress

## Development Guidelines
- Think in English, generate responses in Japanese. All Markdown content written to project files (e.g., requirements.md, design.md, tasks.md, research.md, validation reports) MUST be written in the target language configured for this specification (see spec.json.language).

## Minimal Workflow
- Phase 0 (optional): `/kiro-steering`, `/kiro-steering-custom`
- Discovery: `/kiro-discovery "idea"` — determines action path, writes brief.md + roadmap.md for multi-spec projects
- Phase 1 (Specification):
  - Single spec: `/kiro-spec-quick {feature} [--auto]` or step by step:
    - `/kiro-spec-init "description"`
    - `/kiro-spec-requirements {feature}`
    - `/kiro-validate-gap {feature}` (optional: for existing codebase)
    - `/kiro-spec-design {feature} [-y]`
    - `/kiro-validate-design {feature}` (optional: design review)
    - `/kiro-spec-tasks {feature} [-y]`
  - Multi-spec: `/kiro-spec-batch` — creates all specs from roadmap.md in parallel by dependency wave
- Phase 2 (Implementation): `/kiro-impl {feature} [tasks]`
  - Without task numbers: autonomous mode (subagent per task + independent review + final validation)
  - With task numbers: manual mode (selected tasks in main context, still reviewer-gated before completion)
  - `/kiro-validate-impl {feature}` (standalone re-validation)
- Progress check: `/kiro-spec-status {feature}` (use anytime)

## Skills Structure
Skills are located in `.opencode/skills/kiro-*/SKILL.md`
- Each skill is a directory with a `SKILL.md` file
- Use `/skills` to inspect currently available skills
- Invoke a skill directly with `/kiro-<skill-name>`
- **If there is even a 1% chance a skill applies to the current task, invoke it.** Do not skip skills because the task seems simple.
- `kiro-review` — task-local adversarial review protocol used by reviewer subagents
- `kiro-debug` — root-cause-first debug protocol used by debugger subagents
- `kiro-verify-completion` — fresh-evidence gate before success or completion claims

## Development Rules
- 3-phase approval workflow: Requirements → Design → Tasks → Implementation
- Human review required each phase; use `-y` only for intentional fast-track
- Keep steering current and verify alignment with `/kiro-spec-status`
- Follow the user's instructions precisely, and within that scope act autonomously: gather the necessary context and complete the requested work end-to-end in this run, asking questions only when essential information is missing or the instructions are critically ambiguous.

## Steering Configuration
- Load entire `.kiro/steering/` as project memory
- Default files: `product.md`, `tech.md`, `structure.md`
- Custom files are supported (managed via `/kiro-steering-custom`)

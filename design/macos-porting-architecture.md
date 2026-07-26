# MDLuma macOS Porting Architecture

## Purpose

This document defines the architecture for supporting macOS while preserving MDLuma's Windows support.

It is a design reference for developers and AI agents working on the macOS port. It records the intended responsibility boundaries, known implementation constraints, and migration order. It does not itself authorize implementation changes.

## Product Constraints

- MDLuma remains a lightweight, read-only Markdown viewer.
- Markdown rendering remains `Markdown -> HTML -> Sciter`.
- Rust remains the primary implementation language.
- Comrak remains responsible for GFM-oriented Markdown to HTML conversion.
- Sciter.js SDK remains responsible for HTML/CSS/JavaScript UI rendering.
- Windows 10/11 support must not regress.
- The first macOS target is Apple Silicon: `aarch64-apple-darwin`.
- macOS uses the native title bar and native window controls. It must not emulate the Windows custom minimize, maximize, and close controls.
- Signing and notarization are intentionally outside the first architectural migration. The result must nevertheless be packageable as a standard `.app` bundle.

## Current State

The project is currently a single Rust crate. Much of the viewer logic is already portable, but startup assembly and the Sciter boundary are substantially Windows-specific.

### Already Portable Components

The following responsibilities are OS-independent and should stay shared:

- `src/document.rs`: local UTF-8 document reading and absolute-path resolution.
- `src/markdown.rs`: Comrak configuration and HTML fragment generation.
- `src/html_shell.rs`: UI shell assembly and state-to-HTML rendering.
- `src/html_sanitizer.rs`: local resource URL handling and rendered HTML sanitization.
- `src/open_paths.rs`: dropped-file selection and child-window planning.
- `src/startup_args.rs`: command-line parsing and initial launch planning.
- `src/external_editor.rs`: process spawning contract.
- `src/viewer_launcher.rs`: child viewer process launch contract.
- `src/errors.rs`: user-facing and diagnostic errors.
- `src/ui/mod.rs`: embedded UI assets and theme/icon selection.
- Most of `src/app.rs`: viewer state transitions and command dispatch.

### Windows-Specific Components

The following code is currently Windows-specific or contains Windows-specific behavior and must be isolated from shared code:

| Area | Current location | Reason |
| --- | --- | --- |
| Windows file dialog | `src/platform/windows_file_dialog.rs` | Uses `GetOpenFileNameW` and Win32 owner HWND handling. |
| Windows font dialog | `src/platform/windows_font_dialog.rs` | Uses `ChooseFontW`, GDI, and Win32 owner HWND handling. |
| Window controls | `src/platform/windows_window_chrome.rs` | Uses User32, DWM, and Win32 window state APIs. |
| Browser launch | `src/platform/windows_browser.rs` | Uses `ShellExecuteW`. |
| Runtime loading | `src/sciter/ffi.rs` | Uses `LoadLibraryW`, `GetProcAddress`, and `FreeLibrary`. |
| Custom frame | `src/sciter/ffi.rs` | Removes Win32 window styles and manipulates DWM corners. |
| Deferred HTML loading | `src/sciter/ffi.rs` | Uses a subclassed WndProc and `WM_APP`. |
| Native drag and drop fallback | `src/sciter/ffi.rs` | Uses `WM_DROPFILES`, `DragQueryFileW`, and `DragAcceptFiles`. |
| Keyboard shortcut fallback | `src/sciter/ffi.rs` | Uses Win32 key messages and `GetKeyState`. |
| Window placement persistence | `src/sciter/ffi.rs`, `src/app.rs` | Uses `GetWindowPlacement`, `SetWindowPlacement`, and `GetWindowRect`. |
| Windows-only startup assembly | `src/lib.rs` | Hard-codes `WindowsFileDialog` and `WindowsFontDialog`. |
| Windows resource packaging | `build.rs` | Embeds `.ico` metadata and copies `sciter.dll`. |
| Windows default target | `.cargo/config.toml` | Sets `x86_64-pc-windows-msvc`. |
| Windows-only release workflow | `.github/workflows/release.yml` | Builds and packages only a Windows ZIP distribution. |
| Windows title bar UI | `src/ui/index.html`, `src/ui/styles.css`, `src/ui/app.js` | Uses extended frame, caption roles, and custom window control buttons. |
| Windows application-data paths | `src/settings.rs`, `src/debug_log.rs` | Uses `LOCALAPPDATA` directly. |
| Default external editor | `src/app.rs` | Defaults to `notepad.exe`. |

## Architecture Decision

Keep a single crate. Do not create a workspace or split this project into multiple crates during the first macOS port.

The project is small and startup/memory cost matter. A single crate with strict module boundaries provides the required separation without introducing package-management overhead. Create additional crates only if a later platform implementation needs independent release, reuse, or substantially different build dependencies.

The intended dependency direction is:

```text
core application logic
        |
        v
platform contracts <--- Sciter UI adapter
        |
        v
platform implementations (Windows or macOS)
```

Shared application logic may depend only on contracts. It must not import Windows or macOS modules, platform `cfg` branches, native handles, native APIs, or platform runtime filenames.

## Target Module Layout

The layout below is the target organization. Incremental migration is preferred; files do not need to move until their responsibility is extracted.

```text
src/
  app.rs
  document.rs
  markdown.rs
  html_shell.rs
  html_sanitizer.rs
  settings.rs
  startup_args.rs
  open_paths.rs
  errors.rs
  external_editor.rs
  viewer_launcher.rs
  ui/

  platform/
    mod.rs
    contracts.rs
    paths.rs
    windows/
      mod.rs
      file_dialog.rs
      font_dialog.rs
      browser.rs
      paths.rs
    macos/
      mod.rs
      file_dialog.rs
      font_dialog.rs
      browser.rs
      paths.rs

  sciter/
    mod.rs
    api.rs
    runtime.rs
    runtime_assets.rs
    window.rs
    loader/
      mod.rs
      windows.rs
      macos.rs
    windows.rs
    generated_sciter_bindings.rs
```

`core/` is a conceptual boundary, not a mandatory directory in this migration. Existing portable top-level modules should remain where they are unless moving them materially improves clarity.

## Platform Contracts

Place platform-neutral traits and result types in `src/platform/contracts.rs`. Their inputs and outputs must be Rust standard-library types or project domain types, not OS-specific types.

The existing contracts are suitable starting points:

- `FileDialog` and `OpenFileResult`
- `FontDialog` and `FontDialogResult`
- `WindowChromeController` only where custom title bars require it
- Browser opening service
- Application-data and log-directory resolver
- Default document opener / external editor service

### Native Window Ownership

Native dialogs require an owner window, but `SciterWindowHandle` is an opaque pointer rather than a portable native window type. Keep the opaque handle at the Sciter/platform boundary only.

- `ViewerUi::native_window_handle()` may expose an opaque handle to platform adapters.
- `AppController` must not interpret the handle.
- Each platform adapter owns conversion and validation of that handle.
- A macOS adapter may convert it to the applicable Cocoa window only inside `platform/macos`.

### Startup Composition

`src/lib.rs` currently fixes the startup controller to `WindowsFileDialog` and `WindowsFontDialog`. Replace this with a platform-selected composition root.

The composition root is the only layer permitted to select platform implementations through `cfg(windows)` or `cfg(target_os = "macos")`. It constructs the same generic `AppController` with platform-specific implementations of the shared contracts.

Do not put platform selection into `AppController`, Markdown conversion, HTML shell generation, or UI state transitions.

## Sciter Boundary

### Shared Sciter Responsibilities

The following Sciter APIs and behaviors are intended to be shared by Windows and macOS:

- Dynamic runtime validation and Sciter version checking.
- Sciter API table lookup through the `SciterAPI` export.
- Window creation through `SciterCreateWindow`.
- HTML loading through `SciterLoadHtml`.
- Sciter script runtime configuration.
- Event handler attachment and `xcall()` command delivery.
- DOM behavior events and external-link routing.
- Sciter exchange drag-and-drop events.
- Theme and font updates through Sciter DOM APIs.
- Application event loop through `SciterExec` after its cross-platform behavior is verified.

The generated `ISciterAPI` bindings must be treated as an ABI boundary. Before using them on macOS, validate the API table with the macOS runtime by loading it, resolving `SciterAPI`, and querying the Sciter version. Do not assume the currently generated bindings are correct for a new SDK revision without that smoke test.

### Runtime Loader Split

Separate dynamic library loading from the Sciter API table wrapper:

- Windows loader: `LoadLibraryW`, `GetProcAddress`, `FreeLibrary`.
- macOS loader: `dlopen`, `dlsym`, `dlclose`.

Both loaders expose the same minimal internal operation: resolve the `SciterAPI` export from an already validated runtime path. The API table wrapper then performs all common function-pointer validation.

Do not use a static link to Sciter. MDLuma continues to distribute Sciter as a bundled runtime.

### Windows-Only Sciter Workarounds

Move the following behavior out of the generic Sciter API/window implementation into `src/sciter/windows.rs`:

- Win32 WndProc subclassing.
- `WM_APP` deferred HTML loading.
- Win32 title-bar style removal.
- DWM window-corner configuration.
- `WM_DROPFILES` fallback handling.
- Win32 wheel delta normalization.
- Win32 keyboard-message fallback for external editor launch.
- Win32 placement capture and restore.
- Win32 title-bar right-click bridge for recent files.

These are Windows compatibility workarounds, not cross-platform Sciter requirements. macOS must not receive no-op copies of this behavior merely to preserve a shared call path.

### Drag and Drop

Use Sciter's DOM/exchange drag-and-drop support as the portable path. The SDK sample `samples.sciter/drag-n-drop-system/shell-file-drop.htm` demonstrates accepting shell file drops through `ondragaccept` and reading `evt.detail.data`.

The current `FileDropTarget` in `src/ui/app.js` already follows this Sciter model. Preserve and test it on macOS. Keep the native `WM_DROPFILES` path as Windows-only fallback because it exists to work around a Windows-specific failure after DOM reconstruction.

### File URLs

Keep `file_url_from_path()` portable. Its Windows normalization is appropriately isolated under `cfg(windows)`. The inverse conversion used for dropped Sciter file URIs must be redesigned to use URI-aware conversion rather than Windows slash replacement. It must correctly support POSIX absolute paths and percent-encoded path segments on macOS.

## macOS UI Policy

### Standard Window Frame

macOS uses a standard native window title bar. Do not use the Windows extended-frame configuration on macOS.

On macOS, the HTML shell must omit:

- Custom minimize button.
- Custom maximize/restore button.
- Custom close button.
- `role="window-caption"` drag-region behavior.
- Windows-specific frame spacing and maximized-window CSS.

The application toolbar remains shared and retains:

- Application identity and document name where appropriate.
- Open file action.
- Search action.
- Theme action.
- More menu, font setting, external editor action, and About action.

### UI Rendering Variant

Add a platform-neutral shell model value, for example `WindowPresentation`, rather than placing `cfg` conditions inside HTML asset strings.

Suggested values:

- `CustomFrame`: Windows current experience.
- `NativeFrame`: macOS standard title bar.

`DefaultHtmlShell` selects the appropriate structural fragments from this value. The domain state remains unchanged.

### Fonts and Shortcuts

- Replace hard-coded Windows-only font fallbacks such as `Segoe UI`, `Cascadia Code`, and `Consolas` with ordered cross-platform fallback lists.
- Preserve `metaKey` shortcut handling in the Sciter script. It is required for Command-key shortcuts on macOS.
- Retain Sciter-specific APIs and event behavior. Do not rewrite UI code based on browser/WebView assumptions.

## macOS Platform Behavior

### Dialogs

- Markdown open dialog: use Cocoa `NSOpenPanel`, constrained to supported Markdown extensions as appropriate.
- Font selection: use the macOS font panel/manager. Preserve the shared contract of selected family name plus point size in tenths.
- External editor selection: if retained, accept application bundles and appropriate executable forms rather than filtering only for `.exe`.

### Browser and External Editor

- Open external `http` and `https` links using macOS workspace/URL APIs.
- Do not default to a hard-coded editor executable on macOS.
- The preferred macOS default is opening the current Markdown document with its system default application.
- A user-selected editor remains an optional cross-platform setting. Its representation must support a macOS `.app` bundle, not only a native executable file.

### Settings and Logs

The current use of `LOCALAPPDATA` must be replaced by a platform path resolver:

| Data | Windows | macOS |
| --- | --- | --- |
| Settings | `%LOCALAPPDATA%\\MDLuma\\settings.json` | `~/Library/Application Support/MDLuma/settings.json` |
| Debug logs | `%LOCALAPPDATA%\\MDLuma\\logs\\` | `~/Library/Logs/MDLuma/` |

Keep temporary-directory fallback behavior for path-resolution failures and test the resolver independently from filesystem writing.

### Window Geometry

The persisted `WindowGeometry` data model may remain shared, but collection and restoration are platform services.

- Windows retains its current native placement behavior after it has been isolated.
- macOS must use native screen/window APIs or Sciter-supported window geometry behavior.
- Geometry restoration must verify that the window remains visible on an active display.
- Do not reuse Windows `GetWindowPlacement` assumptions on macOS.

## Runtime and Packaging

### Runtime Files

The runtime filename is platform-specific:

| Platform | Runtime file |
| --- | --- |
| Windows | `sciter.dll` |
| macOS | `libsciter.dylib` |

Rename Windows-specific concepts such as `SCITER_DLL_NAME`, `sciter_dll_path`, and `MissingDll` to runtime-neutral names. User-facing diagnostics must name the actual missing runtime file.

### Apple Silicon Runtime Prerequisite

The currently vendored SDK copy contains `bin/macosx/libsciter.dylib`, but it was inspected from Windows and its architecture has not been verified locally.

Before macOS implementation begins:

1. Obtain the macOS Sciter SDK from the official Sciter GitLab distribution source.
2. On an Apple Silicon Mac, run `lipo -info` on `libsciter.dylib`.
3. Confirm that it contains `arm64` and that its redistribution license permits the intended package.
4. Run a runtime-load smoke test against that exact binary.

The Sciter GitLab `bin/macosx` directory currently publishes `libsciter.dylib`. Architecture and ABI must still be checked from the downloaded artifact, not inferred from its filename or repository path.

### App Bundle Layout

The initial macOS packaging target is:

```text
MDLuma.app/
  Contents/
    Info.plist
    MacOS/
      mdluma
    Frameworks/
      libsciter.dylib
    Resources/
      AppIcon.icns
```

The build/package step must ensure the executable resolves the bundled Sciter library. Verify the installed-path dependency with `otool -L` on macOS. The application icon must be converted from the existing source icon to `.icns` as part of the packaging pipeline.

`build.rs` remains responsible only for build-time concerns. Keep the Windows resource embedding branch under `cfg(windows)` and add a distinct macOS packaging path rather than overloading Windows assumptions.

## Build and CI Policy

- Do not remove the existing Windows target configuration until CI has a deliberate target-selection replacement.
- Local macOS development uses `cargo build --target aarch64-apple-darwin`.
- Release CI becomes a platform matrix with separate Windows and macOS jobs.
- The Windows job continues to package `mdluma.exe` with `sciter.dll`.
- The macOS job builds the `.app` bundle with `libsciter.dylib` and `AppIcon.icns`.
- Signing and notarization are added in a later delivery-specific change. Keep signing credentials and release secrets out of source control and generic build scripts.

## Migration Plan

### Phase 0: Establish macOS Runtime Evidence

1. Download the macOS Sciter SDK on Apple Silicon.
2. Verify `libsciter.dylib` architecture with `lipo -info`.
3. Verify the license and redistribution requirements for the exact SDK version.
4. Add a minimal macOS runtime smoke program or test that loads the library, resolves `SciterAPI`, and reads its version.
5. Record the validated Sciter SDK revision and runtime architecture in the macOS implementation specification.

No Cocoa UI implementation should begin before this phase passes.

### Phase 1: Extract Platform Contracts

1. Move `FileDialog` and `OpenFileResult` from the Windows file-dialog module into platform contracts.
2. Move `WindowChromeController` and shared state types out of the Windows implementation if Windows still requires them.
3. Add path-resolution, URL-opening, and default-document-opening platform contracts.
4. Change `SettingsFile` and debug logging to obtain directories through platform contracts.
5. Refactor `lib.rs` startup assembly so it does not name Windows implementation types in shared aliases or generic constraints.
6. Preserve all existing Windows behavior and unit tests during this extraction.

### Phase 2: Separate Common Sciter From Win32

1. Split the dynamic-loader implementation from `SciterApi`.
2. Remove Windows conditional compilation from common API table members that are part of the cross-platform Sciter C API.
3. Move all WndProc and Win32-message behavior into a Windows-only adapter.
4. Make common `SciterWindow` depend on a small platform window adapter where necessary.
5. Make Sciter exchange DnD and DOM command handling work without Windows-only code paths.
6. Remove non-Windows placeholder errors only after a macOS runtime smoke test proves the common path works.

### Phase 3: Implement macOS Adapters

1. Implement macOS dynamic runtime loading.
2. Implement macOS file dialog, font selection, browser launch, and data-directory resolution.
3. Implement macOS default document opening and optional editor bundle selection.
4. Implement macOS geometry capture/restore only after the basic viewer is stable.
5. Add integration tests for each adapter where native behavior can be tested, and isolate remaining manual checks.

### Phase 4: Introduce Native-Frame UI Variant

1. Add `WindowPresentation` to the HTML shell model.
2. Keep the current Windows custom-frame markup and behavior under `CustomFrame`.
3. Add a `NativeFrame` shell path that omits Windows title-bar elements.
4. Remove macOS dependence on `WindowChromeController` and Win32 caption event workarounds.
5. Verify Open, search, theme, recent files, text selection/copy, DnD, and external links in both variants.

### Phase 5: Package and Validate macOS

1. Build the `.app` bundle.
2. Bundle `libsciter.dylib` with a verified install name/path.
3. Add `Info.plist` and an `.icns` application icon.
4. Add macOS CI on Apple Silicon or an appropriate native runner.
5. Run the full regression matrix described below.

## Verification Matrix

### Shared Automated Tests

- Markdown GFM rendering and sanitization.
- Document path normalization and local relative-resource resolution.
- Startup argument parsing and multiple-document child launch planning.
- Viewer state transitions, recent files, theme persistence, and error presentation.
- Platform contract behavior using fakes.
- HTML shell output for both `CustomFrame` and `NativeFrame` variants.

### Windows Regression Tests

- Runtime validation with `sciter.dll` next to the executable.
- Existing custom title bar controls and caption drag behavior.
- Native `WM_DROPFILES` fallback after document DOM replacement.
- Window geometry persistence and restored placement.
- Win32 file dialog, font dialog, browser launch, and external editor behavior.

### macOS Integration and Manual Tests

- `libsciter.dylib` loads on `aarch64-apple-darwin` and reports the expected Sciter version.
- A bundled `.app` launches without relying on the build directory.
- Standard macOS title bar and window controls behave normally.
- Open Panel selects a Markdown document.
- A document loaded by command line, file dialog, and drag-and-drop displays correctly.
- Sciter DOM/exchange drag-and-drop continues to work after loading multiple documents.
- Command-O, Command-F, Command-C, Command-W, Escape, and text search behave as intended.
- Theme, body font, recent files, settings, and debug logs persist in macOS locations.
- External `http` and `https` links open through the system browser.
- Default document opening and configured external editor behavior work with macOS application bundles.
- Multi-file launch creates the intended viewer instances without relying on Windows-only geometry logic.

## Known Verification Gap

At the time this document was written, `cargo check` succeeds on the Windows development environment. `cargo test` compiles and runs Rust tests, but 35 UI JavaScript asset tests fail because Node.js is not installed in that environment. This is an environment prerequisite issue, not evidence of a macOS implementation failure.

Before declaring either platform port complete, install Node.js or revise the JS-test runner policy so all UI script tests execute in CI.

## Non-Goals

- Replacing Sciter with a WebView or browser-based runtime.
- Adding Markdown editing, saving, tabs, or unrelated application features.
- Rewriting portable application logic solely to introduce more abstractions.
- Reimplementing Windows-specific WndProc workarounds on macOS.
- Supporting Intel macOS in the first macOS release.
- Implementing code signing or notarization as part of the architecture extraction itself.

## Rules for Future Changes

- Keep OS-specific APIs inside `src/platform/<os>/`, `src/sciter/loader/<os>.rs`, or explicitly OS-specific Sciter workaround modules.
- Do not add `cfg(windows)` branches to shared application logic when a platform contract can express the behavior.
- Do not expose Win32, Cocoa, HWND, or NSWindow types from shared traits.
- Keep the Sciter C API boundary small and verify Sciter behavior against the vendored SDK documentation, samples, and headers before changing UI/runtime integration.
- Prefer native platform behavior over visually identical custom emulation when the chosen product policy differs by OS.
- Preserve unit tests around contracts and add platform-specific integration tests at the adapter boundary.

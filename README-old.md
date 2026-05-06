# MDLuma

MDLuma is a lightweight desktop Markdown viewer written in Rust.

The project is focused on a simple, fast, read-only viewing experience for local Markdown files on Windows. Markdown is converted to HTML with Comrak and rendered with [sciter](https://sciter.com) ([sciter-js-sdk](https://gitlab.com/sciter-engine/sciter-js-sdk)).

## Status

MDLuma is an early-stage project.

Current development is focused on a minimal viewer experience:

- Open a local Markdown file
- Render GitHub Flavored Markdown-style content as formatted HTML
- Show the document in a read-only desktop window
- Keep the UI lightweight and distribution-friendly for Windows

## Goals

- Fast startup
- Low memory usage
- Minimal, viewer-only scope
- Windows 10/11 first, with portability kept in mind for future platforms

## Tech Stack

- Rust
- Comrak for Markdown to HTML conversion
- Sciter.js SDK for HTML/CSS/JS-based desktop UI rendering

## Building

MDLuma currently targets `x86_64-pc-windows-msvc`.

Requirements:

- Rust toolchain
- Windows build tools
- Sciter runtime files available for packaging and local runs

Build:

```bash
cargo build --release --target x86_64-pc-windows-msvc
```

Test:

```bash
cargo test
```

## Developer Resources

For agents and developers working on this project, a copy of the Sciter SDK documentation is available locally at `vendor/sciter-js-sdk-main/docs/`. This directory contains the contents of the `docs/` folder from the Sciter SDK to serve as a convenient, offline reference for Sciter's APIs, behaviors, and specifics.

## Runtime Notes

MDLuma uses the Sciter runtime for its embedded desktop UI.

At runtime, the application expects the required Sciter files and UI assets to be available alongside the packaged application. In this repository, Sciter-related runtime files are kept under `vendor/sciter-js-sdk-main/` for project use.

### Sciter DLL Version Checking

MDLuma performs version checking on the Sciter DLL at startup to ensure compatibility with the API used during build.

- The full DLL version (e.g., `6.0.3.18`) is queried and logged for debugging purposes
- Compatibility is verified against the major and minor version numbers only (e.g., `6.0`)
- If the major or minor version does not match the expected version, MDLuma will fail fast with a user-friendly error message

This approach allows for patch and build number differences while ensuring API-level compatibility.

## License

The MDLuma project source code is dual-licensed under:

- MIT
- Apache License 2.0

You may use this project under either license at your option. See `LICENSE-MIT` and `LICENSE-APACHE` for details.

## Third-Party Runtime: Sciter

MDLuma uses Sciter.js SDK internally for rendering the desktop UI.

Sciter is a third-party component and is not covered by the MDLuma project license. Any Sciter runtime files, SDK files, or related binaries are subject to Sciter's own license and redistribution terms.

If you distribute MDLuma with Sciter runtime files, you are responsible for complying with the applicable Sciter license terms.

For reference, the Sciter SDK distribution includes separate license documents for different parts of Sciter:

- `LICENSE` - Sciter SDK source distribution license (BSD 3-Clause)
- `SCITER-ENGINE-EULA.md` - Sciter Engine runtime license terms for engine binaries such as `sciter.dll`

Refer to the official Sciter SDK download page for the SDK package and its accompanying license files:

- [https://sciter.com/download/](https://sciter.com/download/)

When evaluating redistribution, packaging, or commercial/non-commercial usage conditions, review the Sciter Engine EULA carefully, especially for the runtime binary that is shipped with the application.

## Repository Purpose

This repository is intended to host the open source MDLuma viewer project itself. The project aims to stay small, practical, and easy to understand.

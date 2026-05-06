# Sciter bindgen workflow

This project uses a committed, generated binding file for the Sciter C API.

- Generated file: `src/sciter/generated_sciter_bindings.rs`
- Wrapper header: `tools/bindgen/sciter_wrapper.h`
- Generator script: `tools/bindgen/generate_sciter_bindings.ps1`

## Maintainer workflow

1. Install LLVM/Clang so that `libclang.dll` is available.
2. Install bindgen CLI once:

```powershell
rtk cargo install bindgen-cli
```

3. Regenerate bindings:

```powershell
powershell -ExecutionPolicy Bypass -File "tools/bindgen/generate_sciter_bindings.ps1"
```

4. Validate:

```powershell
rtk cargo check
rtk cargo test
```

## Notes

- Regular `cargo build` does not run bindgen.
- This keeps day-to-day builds on MSVC unchanged.

#[cfg(windows)]
#[path = "src/sciter/runtime_assets.rs"]
mod runtime_assets;

fn set_git_commit_hash() {
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={hash}");
}

#[cfg(windows)]
fn main() {
    set_git_commit_hash();

    let icon_path = runtime_assets::application_icon_path();
    if !icon_path.exists() {
        panic!("missing application icon: {}", icon_path.display());
    }

    println!("cargo:rerun-if-changed={}", icon_path.display());

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(icon_path.to_string_lossy().as_ref());
    resource.set("FileDescription", "MDLuma Markdown Viewer");
    resource.set("ProductName", "MDLuma");
    resource.set("OriginalFilename", "mdluma.exe");
    resource.set(
        "LegalCopyright",
        "Copyright © 2026 Akira Shimosako. Licensed under MIT OR Apache-2.0.",
    );
    resource.compile().expect("compile Windows resources");

    let profile_dir = profile_output_dir();
    copy_sciter_runtime(&profile_dir);
}

#[cfg(not(windows))]
fn main() {
    set_git_commit_hash();
}

#[cfg(windows)]
fn copy_sciter_runtime(profile_dir: &std::path::Path) {
    let runtime_source = std::path::Path::new("vendor")
        .join("sciter-js-sdk-main")
        .join("bin")
        .join("windows")
        .join("x64")
        .join(runtime_assets::SCITER_DLL_NAME);
    if !runtime_source.exists() {
        panic!("missing Sciter runtime: {}", runtime_source.display());
    }

    println!("cargo:rerun-if-changed={}", runtime_source.display());

    let runtime_destination = profile_dir.join(runtime_assets::SCITER_DLL_NAME);

    std::fs::copy(&runtime_source, &runtime_destination).unwrap_or_else(|error| {
        panic!(
            "failed to copy Sciter runtime from {} to {}: {error}",
            runtime_source.display(),
            runtime_destination.display()
        )
    });
}

#[cfg(windows)]
fn profile_output_dir() -> std::path::PathBuf {
    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("Cargo must provide OUT_DIR for build scripts"),
    );
    out_dir
        .ancestors()
        .nth(3)
        .expect("OUT_DIR should be nested under the Cargo profile directory")
        .to_path_buf()
}

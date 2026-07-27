#[cfg(test)]
mod tests;

mod artifact;
mod manifest;
mod model;
mod sciter;

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
compile_error!("macOS runtime evidence toolkit requires a native arm64 target");

fn main() {
    println!("macOS runtime evidence toolkit built for native arm64");
}

#![windows_subsystem = "windows"]

fn main() {
    if let Err(error) = mdluma::run() {
        report_startup_failure(&error.to_string());
        std::process::exit(1);
    }
}

fn report_startup_failure(message: &str) {
    mdluma::debug_log!("startup failure: {message}");
    eprintln!("MDLuma failed to start: {message}");
}

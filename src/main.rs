#![windows_subsystem = "windows"]

fn main() {
    if let Err(error) = mdluma::run() {
        report_startup_failure(&error.to_string());
        std::process::exit(1);
    }
}

fn report_startup_failure(message: &str) {
    mdluma::debug_log!("startup failure: {message}");
    report_startup_failure_with(message, stderr_fallback);
}

fn report_startup_failure_with(message: &str, mut stderr_reporter: impl FnMut(&str)) {
    stderr_reporter(message);
}

fn stderr_fallback(message: &str) {
    eprintln!("MDLuma failed to start: {message}");
}

#[cfg(test)]
mod tests {
    use super::report_startup_failure_with;
    use std::cell::RefCell;

    #[test]
    fn startup_failure_always_reports_to_stderr_fallback() {
        let stderr_messages = RefCell::new(Vec::new());
        report_startup_failure_with(
            "MDLuma cannot start. Diagnostic: missing runtime file: expected C:\\dist\\MDLuma\\sciter.dll",
            |message| stderr_messages.borrow_mut().push(message.to_string()),
        );

        assert_eq!(
            stderr_messages.into_inner(),
            vec![
                "MDLuma cannot start. Diagnostic: missing runtime file: expected C:\\dist\\MDLuma\\sciter.dll"
                    .to_string()
            ]
        );
    }
}

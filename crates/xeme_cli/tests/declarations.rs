use std::io::Write;
use std::process::{Command, Output, Stdio};

fn check(input: &[u8], width: usize) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_xeme"))
        .args(["--events", "--chunk-size", &width.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn declaration_versions_control_exit_status_and_events() {
    for version in ["1.0", "1.2", "1.01", "banana", "2.0", "1", "1."] {
        let input = format!("<?xml version='{version}'?><r/>");
        for width in [1, 7, input.len()] {
            let result = check(input.as_bytes(), width);
            let valid = matches!(version, "1.0" | "1.2" | "1.01");
            assert_eq!(result.status.success(), valid, "{version}: {result:?}");
            if valid {
                assert!(!result.stdout.is_empty());
            } else {
                assert!(result.stdout.is_empty());
                assert!(!result.stderr.is_empty());
            }
        }
    }
}

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
fn utf16_requires_encoding_evidence_before_printing_events() {
    for big_endian in [false, true] {
        for evidence in ["none", "bom", "declaration"] {
            let name = if big_endian { "UTF-16BE" } else { "UTF-16LE" };
            let text = match evidence {
                "bom" => "\u{feff}<r/>".to_string(),
                "declaration" => format!("<?xml version='1.0' encoding='{name}'?><r/>"),
                _ => "<r/>".to_string(),
            };
            let bytes: Vec<u8> = text
                .encode_utf16()
                .flat_map(|unit| {
                    if big_endian {
                        unit.to_be_bytes()
                    } else {
                        unit.to_le_bytes()
                    }
                })
                .collect();
            for width in [1, 7, bytes.len()] {
                let result = check(&bytes, width);
                assert_eq!(result.status.success(), evidence != "none", "{result:?}");
                if evidence == "none" {
                    assert!(result.stdout.is_empty());
                }
            }
        }
    }
}

//! A bounded streaming XML checker.

#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::process::ExitCode;

use xeme::{Config, Parser};

#[cfg(all(feature = "performance-allocator", target_os = "windows"))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(
    feature = "performance-allocator",
    not(target_os = "windows"),
    not(target_os = "openbsd"),
    not(target_os = "freebsd"),
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64"
    )
))]
#[global_allocator]
static ALLOCATOR: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

const HELP: &str = "A streaming XML checker.\n\nUsage: xeme [OPTIONS] [FILE|-]\n\nOptions:\n  --events            Print parser events\n  --namespaces        Expand namespace names with | as separator\n  --chunk-size BYTES  Read at most BYTES per chunk (default: 65536)\n  --help              Print this help\n  --version           Print the version\n\nReads standard input when FILE is omitted. Resource limits use parser defaults.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xeme: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut path: Option<OsString> = None;
    let mut events = false;
    let mut namespaces = false;
    let mut chunk_size = 65_536;
    let mut positional = false;
    while let Some(arg) = args.next() {
        if !positional {
            match arg.to_str() {
                Some("--help" | "-h") => {
                    println!("{HELP}");
                    return Ok(());
                }
                Some("--version" | "-V") => {
                    println!("xeme {}", env!("CARGO_PKG_VERSION"));
                    return Ok(());
                }
                Some("--events") => {
                    events = true;
                    continue;
                }
                Some("--namespaces") => {
                    namespaces = true;
                    continue;
                }
                Some("--") => {
                    positional = true;
                    continue;
                }
                Some("--chunk-size") => {
                    chunk_size = args
                        .next()
                        .and_then(|arg| arg.to_str()?.parse::<usize>().ok())
                        .filter(|size| (1..=16 * 1024 * 1024).contains(size))
                        .ok_or("--chunk-size requires an integer between 1 and 16777216")?;
                    continue;
                }
                Some(option) if option.starts_with('-') && option != "-" => {
                    return Err(format!("unknown option {option:?}").into());
                }
                _ => {}
            }
        }
        if path.replace(arg).is_some() {
            return Err("expected at most one input file".into());
        }
    }
    let mut input: Box<dyn Read> = match path.as_deref() {
        None => Box::new(io::stdin().lock()),
        Some(path) if path == "-" => Box::new(io::stdin().lock()),
        Some(path) => Box::new(File::open(path)?),
    };
    let config = Config {
        namespace_separator: namespaces.then_some('|'),
        ..Config::default()
    };
    let mut parser = Parser::new(config);
    let mut buffer = vec![0; chunk_size];
    let mut output = BufWriter::new(io::stdout().lock());
    loop {
        let count = loop {
            match input.read(&mut buffer) {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        parser.feed(&buffer[..count], count == 0)?;
        while let Some(event) = parser.next_event()? {
            if events {
                writeln!(output, "{event:?}")?;
            }
        }
        if count == 0 {
            break;
        }
    }
    output.flush()?;
    Ok(())
}

# Optional profile-guided builds

Profile-guided optimization (PGO) uses execution counts from a training run to
optimize code. This tool builds an instrumented Xeme library, trains it on
generated XML, merges the profile, and builds an optimized library.

Release builds do not use PGO by default. Benchmark the result on your
application's workload.

For static linking on Linux, `--native-static-libs` captures native linker
dependencies from the same compilation that produces the optimized archive.

## Requirements

- Linux or macOS, Python 3.11 or later, and an installed native Rust toolchain.
- An installed `llvm-profdata` with the same LLVM major, minor, and patch version reported by `rustc -vV`.
- Xeme's Cargo dependencies already cached. Cargo runs with `--locked --offline`.
- Separate, non-overlapping source and output directories. Use one output directory per build configuration.

Run from the repository, supplying paths appropriate to your machine:

```sh
uv run --offline tools/pgo/build.py \
  --source "$PWD" \
  --output "$PWD/../xeme-pgo-output" \
  --llvm-profdata /path/to/llvm-profdata
```

Select an installed toolchain with `--toolchain stable` or, for local development,
`--toolchain ohm`. Production builds should use the project's normal toolchain.
The script builds for the compiler's host target. It rejects compiler-wrapper
environment settings and disables wrappers from Cargo configuration. Existing
Rust flags are retained, but inherited PGO flags are rejected. Each command has a
30-minute timeout, configurable with `--command-timeout`.

Both phases select `--lib --crate-type cdylib,staticlib` through Cargo's `rustc`
command. This enables the release profile's ThinLTO step for the C artifacts while
keeping `rlib` available for Rust tests and consumers. Build logs retain compiler
commands. A matching normal C build uses:

```sh
cargo rustc --release --locked --target YOUR_HOST_TARGET -p xeme_expat --lib --crate-type cdylib,staticlib
```

Global Cargo options can be repeated with `--cargo-arg=OPTION`, attaching any option
value in the same argument. For local Ohm validation with its experimental defaults
disabled, use `--toolchain ohm --cargo-arg=-Zohm-defaults=no`. These options are
passed before Cargo's operation. Directory-changing options are rejected.
Configuration overrides must use inline `--config=KEY=VALUE` syntax; additional
configuration files are not accepted.

## Outputs

Each invocation creates a `runs/run-*` directory with fresh profiles and libraries.
The `targets/generate` and `targets/use` directories cache Cargo build dependencies.

A successful run contains:

- `use/libxeme_expat.so` (Linux) or `.dylib` (macOS), and `use/libxeme_expat.a`.
- The instrumented libraries, raw profiles, merged profile, and complete profile counter dump.
- Generated XML inputs, their manifest, and instrumented and optimized parse records.
- `manifest.json` with build inputs, hashes, commands, selected environment
  variables, exit codes, timeouts, and log hashes.
- With `--native-static-libs`, the native linker dependencies from the optimized
  build.

`latest.json` points to the last successful manifest and records its hash. Failed
runs retain their manifest and logs. A `.lock` prevents simultaneous use of the
output directory. If the build is killed, check that the recorded process and its
compiler children have stopped before removing a stale lock.

## Training

The corpus is deterministic and generated locally. It covers UTF-8, UTF-16,
Latin-1, namespaces, DTD declarations, attributes, references, comments,
processing instructions, CDATA, nesting, and line endings. Each fixture runs twice
with fresh parsers at three chunk widths, in both namespace modes and with
minimal/full callback sets. It includes no external-file callbacks or real
project inputs.

Training runs Python with `-I -S` and verifies the loaded `XML_Parse` address with
`dladdr`. Python `ctypes` callbacks record event digests and counts; only the Rust
library is instrumented. Element declaration models are freed, but their internal
trees are not included in the digest.

The tool rejects version mismatches, compiler warnings during profile use,
incomplete training, changed build inputs, and differing replay records between
the instrumented and optimized libraries.

## Tool checks

```sh
python3 -m unittest discover -s tools/pgo -p 'test_*.py'
ruff check tools/pgo
ruff format --check tools/pgo
ty check --extra-search-path tools tools/pgo
```

See the [Rust compiler PGO documentation](https://doc.rust-lang.org/rustc/profile-guided-optimization.html)
for background. Regenerate profiles when source, compiler, target, or build flags
change.

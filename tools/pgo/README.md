# Optional profile-guided builds

Profile-guided optimization (PGO) lets the compiler use execution counts from a training run to lay out and optimize code. This tool builds an instrumented Oriole library, trains it on generated XML, merges the profile, and builds an optimized library. It does not change the parser's allocator, limits, or runtime configuration.

This workflow is separate from the default release and python-build-standalone builds. Evaluate the resulting library with your application's tests and benchmarks before deployment.

For a PBS consumer, use the [optional PGO bundle command](../../integration/python-build-standalone/#optional-fresh-pgo-bundle). It runs this pipeline with the required PIC/unwind flags, verifies the effective ThinLTO configuration, and packages the optimized archive with its build manifest and native linker dependencies. The `--native-static-libs` option is used by that Linux bridge to capture native dependencies from the same profile-use compilation.

## Requirements

- Linux or macOS, Python 3.11 or later, and an installed native Rust toolchain.
- An installed `llvm-profdata` with the same LLVM major, minor, and patch version reported by `rustc -vV`.
- Oriole's Cargo dependencies already cached. Cargo runs with `--locked --offline`; the tool downloads neither toolchains nor XML.
- Separate, non-overlapping source and output directories. Use one output directory per build configuration.

Run from the repository, supplying paths appropriate to your machine:

```sh
uv run --offline tools/pgo/build.py \
  --source "$PWD" \
  --output "$PWD/../oriole-pgo-output" \
  --llvm-profdata /path/to/llvm-profdata
```

For an already installed alternate toolchain, add `--toolchain stable` or, for local development, `--toolchain ohm`. Production builds should use the project's normal toolchain. The script explicitly builds for that compiler's host target and rejects nonempty compiler-wrapper environment settings. It also explicitly disables wrappers supplied through Cargo configuration, so the recorded compiler is invoked directly. Existing Rust flags are retained using Cargo's encoded flag format, including arguments containing spaces; inherited PGO flags are rejected. Each command has a 30-minute timeout, configurable with `--command-timeout`.

Both phases select `--lib --crate-type cdylib,staticlib` through Cargo's `rustc`
command. This enables the release profile's ThinLTO step for the C artifacts while
keeping `rlib` available in the manifest for Rust tests and consumers. Verbose build
logs retain the actual compiler commands. A matching normal C build uses:

```sh
cargo rustc --release --locked --target YOUR_HOST_TARGET -p oriole_expat --lib --crate-type cdylib,staticlib
```

Global Cargo options can be repeated with `--cargo-arg=OPTION`, attaching any option
value in the same argument. For local Ohm validation with its experimental defaults
disabled, use `--toolchain ohm --cargo-arg=-Zohm-defaults=no`. These options are
recorded and passed before Cargo's operation, separately from Rust compiler flags.
Directory-changing options are rejected. Configuration overrides must use inline
`--config=KEY=VALUE` syntax; additional configuration files are not accepted.

## Outputs

Every invocation creates a new `runs/run-*` directory with an empty raw-profile directory. The stable `targets/generate` and `targets/use` directories reuse ordinary Cargo build dependencies, while each run's unique profile paths force fresh instrumented and optimized compilation. Profiles are never reused across source or compiler changes.

A successful run contains:

- `use/liboriole_expat.so` (Linux) or `.dylib` (macOS), and `use/liboriole_expat.a`.
- The instrumented libraries, raw profiles, merged profile, and complete profile counter dump.
- Generated XML inputs, their manifest, and instrumented and optimized parse records.
- `manifest.json` with source, Cargo configuration, script, tool, input, profile, and library hashes; exact commands, selected build environment, exit codes, timeouts, and raw log hashes.

`latest.json` points to the last successful manifest and records its hash. Failed runs retain their manifest and logs without replacing that pointer. An exclusive `.lock` prevents simultaneous use of the output directory. After a killed orchestrator, remove a stale lock only after confirming its recorded process and compiler children have stopped.

The training corpus is deterministic and generated locally. It covers UTF-8, UTF-16, Latin-1, namespaces, DTD declarations, attributes, references, comments, processing instructions, CDATA, nesting, and line endings. Each fixture runs twice with fresh parsers at three chunk widths, in both namespace modes and with minimal/full callback sets. The training process uses Python `-I -S` to disable user paths and site initialization, imports only its own helper module, and checks the loaded `XML_Parse` address with `dladdr`. Python `ctypes` callbacks record event digests and counts; Python itself is not instrumented. Element declaration models are freed but their internal trees are not part of the digest. There are no external-file callbacks or real project inputs.

Commands inherit the caller's environment with the recorded build overrides. The manifest records an allowlist of build variables, not credentials or a complete environment snapshot.

The tool rejects version mismatches, compiler warnings during profile use, incomplete training, changed build inputs, and differing generated replay records. It does not guarantee bit-for-bit reproducible binaries across paths, machines, or compilers, or performance on workloads absent from training.

## Tool checks

```sh
python3 -m unittest discover -s tools/pgo -p 'test_*.py'
ruff check tools/pgo
ruff format --check tools/pgo
ty check --extra-search-path tools tools/pgo
```

The workflow follows the build/train/merge/use sequence described in the [Rust compiler PGO documentation](https://doc.rust-lang.org/rustc/profile-guided-optimization.html). Regenerate profiles whenever source, compiler, target, or relevant build flags change.

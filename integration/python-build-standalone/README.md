# Experimental python-build-standalone integration

This recipe overlays Oriole's static archive and public header in the CPython build
container for [PBS revision `a4553880`](https://github.com/astral-sh/python-build-standalone/tree/a4553880293fe9d1bb62747d34ab0e5121d3554f).
It is opt-in and supports CPython **3.12.13**, a Linux x86_64 host,
the generic `x86_64-unknown-linux-gnu` target, an explicit
`x86_64_v3-unknown-linux-gnu` opt-in, and Docker builds of shared Python.

The ordinary Expat dependency still builds and remains in PBS's dependency cache.
The overlay replaces it only inside the selected CPython build container. A phony
Make dependency forces CPython to rebuild whenever the overlay is selected. Use a
separate PBS checkout and output directory for this experiment: resulting Python
archives retain PBS's normal filenames and must not enter the release artifact pool.

## Prepare a bundle

From an Oriole checkout, with Rust 1.96 or newer and its matching documentation
component installed:

```sh
python3 integration/python-build-standalone/prepare.py --output /absolute/oriole-bundle
```

For local Ohm development, add `--toolchain ohm --cargo-arg=-Zohm-defaults=no` and set a separate
`CARGO_TARGET_DIR` and shared `CARGO_BUILD_BUILD_DIR` as instructed by the workspace.
The output directory must not already exist. A failed build leaves its log there;
use a new output directory when retrying.

Both normal and PGO bundles reject inherited Rust flags, compiler wrappers,
Cargo profile environment overrides and configured Rust flags. Both verify all
three fresh workspace compiler vectors, including the ABI target, CPU selection,
PIC, unwinding, ThinLTO and one codegen unit. Use a fresh `CARGO_TARGET_DIR` for a
normal build so cached compilations cannot omit this evidence.

### Explicit x86-64-v3 trial

Generic remains the default. To select a distribution that requires x86-64-v3:

```sh
python3 integration/python-build-standalone/prepare.py \
  --output /absolute/oriole-v3-bundle --pbs-target x86_64_v3-unknown-linux-gnu
bash integration/python-build-standalone/run.sh \
  /absolute/pbs-oriole /absolute/oriole-v3-bundle x86_64_v3-unknown-linux-gnu
```

The same `--pbs-target` option combines with `--pgo --llvm-profdata ...`. PBS uses
its existing v3 target for the whole distribution; Rust retains the generic ABI
triple `x86_64-unknown-linux-gnu` and adds only `-C target-cpu=x86-64-v3`. The bundle
records `target`, `rust_target`, `target_cpu`, actual compiler vectors and host
checks. A v3 bundle passed to the two-argument, generic `run.sh` invocation is
rejected. Unknown targets and hidden CPU/feature overrides are rejected too.

Before training or execution, the v3 path compiles and runs a small GCC CPU/OS
guard with fixed `-march=x86-64 -mtune=generic` flags and a clean environment.
Inherited `CC`, `CFLAGS` and compiler-search overrides do not affect that guard.
Its GCC builtin checks the complete v3 level, including usable AVX OS state;
checking only `avx2` would be insufficient. The generic path does not compile a
guard. A supported CPU does not establish old-glibc compatibility: the actual
distribution still needs the glibc 2.17 and threaded-TLS gates below.

The manual workflow's `pbs_target` choice defaults to generic. Artifact names
include the product target and normal/PGO mode. Installed provenance checks bind
the distribution filename, `PYTHON.json.target_triple`, installed bundle manifest
and exact static archive before running the installed parser. PBS's v3 archive
filename expresses its CPU requirement, but does not identify Oriole by itself;
these experimental artifacts must remain outside release pools.

This wiring enables a controlled trial. Local v3 PGO Python results do not
establish installed-distribution performance, native parity, complete
compatibility or readiness for default deployment.

The script builds with position-independent code and unwinding enabled, captures
`rustc --print=native-static-libs`, and rejects source changes during compilation.
It uses Cargo's `rustc --lib --crate-type cdylib,staticlib` target override so the
release profile's ThinLTO setting applies to the C artifacts. The manifest retains
`rlib` for Rust tests; verbose build logs record the compiler's effective options.
The recipe CI job also builds this bundle with stable Rust and its documentation
component, exercising native-library extraction and the weak TLS-hook check.
The bundle contains:

- `libexpat.a`: Oriole's Rust static archive under the dependency's expected name.
- `expat.h`: the matching narrow-character C header.
- `expat.pc`: static linker metadata. Its version identifies the Expat API target;
  the `implementation` variable identifies Oriole.
- `native-static-libs.txt`: native libraries reported by this exact Rust toolchain.
- `LICENSE.oriole.txt`: Oriole, dependency, Expat-header, and Rust runtime notices.
- `cpython-external-parser.patch`: the upstream CPython child-parser cleanup fix,
  backported to 3.12.13 with recorded source and patch hashes.
- `manifest.json`: source and file hashes, PBS/Rust/CPU targets, toolchain, actual
  compiler vectors, host check and build command.

### Optional fresh PGO bundle

The command above uses the normal ThinLTO release build. To train and bundle a
profile-guided build, provide an installed `llvm-profdata` matching the compiler's
LLVM major, minor, and patch version:

```sh
python3 integration/python-build-standalone/prepare.py \
  --output /absolute/oriole-pgo-bundle \
  --pgo --llvm-profdata /absolute/matching/llvm-profdata
```

The output must be outside the source tree. Cargo dependencies must already be
cached. Unset inherited `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, and `CARGO_PROFILE_*`
overrides: this bundle verifies PIC, unwinding, ThinLTO, one codegen unit, and the
GNU x86-64 target in the actual compiler commands. Use the normal production
toolchain and matching Rust documentation. For local Ohm checks, add
`--toolchain ohm --cargo-arg=-Zohm-defaults=no` and keep its experimental trust
settings disabled.

This delegates to the [existing generated-only PGO pipeline](../../tools/pgo/),
then copies its exact optimized static archive into `libexpat.a`. It captures
native linker libraries from that same profile-use compilation. The source,
compiler, Cargo configuration, training inputs, profiles, library origins, and
generated callback results must still match when packaging completes. No prior
profile or separately rebuilt archive is accepted. The normal TLS, header,
license, and CPython cleanup-backport checks also apply.

`manifest.json` embeds the PGO manifest and its checksum. The `pgo/` subdirectory
retains the complete build logs, generated training records, profiles, and
instrumented and optimized libraries; retain it with the bundle when archiving
evidence. PBS installs the same six payload files and retains the combined
manifest in its license directory. Pass this bundle to `run.sh` exactly as below.

Selecting PGO does not select PBS's CPython optimization variant, and local
benchmark gains do not establish the installed distribution's performance.
Repeat the full distribution, installed XML, glibc 2.17, threaded-TLS, and consumer
benchmark gates for the resulting bundle. The ordinary CI distribution and
default `prepare.py` invocation continue to use the normal release build.

The manual **PBS distribution** workflow accepts a boolean `pgo` input, defaulting
to `false`. Enabling it installs stable Rust's matching LLVM tools, fetches the
locked dependencies before offline training, and passes the resulting PGO bundle
through the same distribution gates. Its validation artifact retains the PGO
logs, manifests, training inputs and records, and profiles. Cargo target trees and
duplicate library binaries are excluded from that artifact. The bundle manifest
retains the exact static archive hash; the produced PGO distribution still needs
its packaged archive identity checked.

PBS pull requests use the normal build, including branches whose names contain
`pbs-pgo-`. The current workflow enables PGO only through its manual `pgo` input.
Manual dispatch requires the workflow to be present on the repository's default
branch.

The archive is built for the GNU target, but target compatibility remains a PBS
validation gate. At the pinned revision, PBS links x86_64 against a Debian Jessie
sysroot inside a newer container. A successful link on the development host does
not establish that older glibc baseline. Run PBS's distribution validator and inspect
its dynamic library and symbol-version results before treating the archive as portable.

The overlay requires Rust's TLS destructor hook reference to remain weak, then
uses the linker's `--wrap=__cxa_thread_atexit_impl` option without defining a
wrapper. This selects Rust's existing pthread-key fallback instead of acquiring
a glibc 2.18 version requirement from the Jessie sysroot. No glibc implementation
is replaced. `validate-tls.py` checks real destructors on C-created threads and
verifies the resulting ELF symbols. This workaround is scoped to the pinned
CPython overlay.

## Apply and run

Create an isolated PBS checkout, then apply the small patch:

```sh
git clone https://github.com/astral-sh/python-build-standalone.git /absolute/pbs-oriole
git -C /absolute/pbs-oriole checkout --detach a4553880293fe9d1bb62747d34ab0e5121d3554f
git -C /absolute/pbs-oriole apply --check /absolute/oriole/integration/python-build-standalone/pbs-a455388.patch
git -C /absolute/pbs-oriole apply /absolute/oriole/integration/python-build-standalone/pbs-a455388.patch
bash integration/python-build-standalone/run.sh /absolute/pbs-oriole /absolute/oriole-bundle
```

The wrapper verifies the PBS revision and applied patch, then sets
`PYBUILD_ORIOLE_BUNDLE` for the normal PBS build command. Without that variable,
the patch leaves the existing build path unchanged. The opt-in path rejects other
targets, CPython versions, non-container builds, and fully static Python builds.

PBS's pinned revision normally selects CPython 3.12.14. The overlay instead pins
the [official 3.12.13 source archive](https://www.python.org/ftp/python/3.12.13/Python-3.12.13.tar.xz),
including its size and SHA-256 (`c08bc65a81971c1dd5783182826503369466c7e67374d1646519adf05207b684`).
The shared download metadata keeps the host Python, target Python, and generated
Make targets on 3.12.13. The wrapper checks the selection before starting the build.

At the pinned CPython version, PBS inherits `Modules/Setup.stdlib` flags from
configure. CPython 3.12.13 reads `LIBEXPAT_CFLAGS` and `LIBEXPAT_LDFLAGS` directly;
installing a pkg-config file alone would not add Rust's native dependencies. The
patch sets those variables from the verified bundle before configure runs.
`pyexpat` links the archive and native libraries; `_elementtree` accesses it through
pyexpat's C API capsule, as in upstream CPython.

The overlay also applies the [upstream CPython cleanup fix](consumer-fix/) before
configure. CPython 3.12.13 can crash or decrement the parent reference twice when
an external parser allocation fails, including when linked to reference Expat.
Oriole's resource limits make this failure path relevant. The recipe verifies the
original source hash, applies the backport without fuzzy matching, and verifies
the resulting hash. The ordinary PBS build path does not apply this backport.

The installer verifies every bundled file's hash before modifying the container.
PBS retains the combined notices and build manifest in the distribution's existing
license directory, and its extension metadata references Oriole's notices.

## Validation

Check the patch and staging logic against a clean pinned PBS source tree:

```sh
python3 -m unittest discover -s integration/python-build-standalone -p test_pgo_bundle.py
python3 integration/python-build-standalone/validate.py --pbs /absolute/clean-pbs \
  --cpython /absolute/cpython-3.12.13
```

This checks clean patch application, Python compilation, shell syntax, the
default build path, native-linker propagation, target restrictions, and rejection
of a mismatched header. With `--cpython`, validation also runs the backport shell
block against the pinned consumer source and verifies that reapplication is
rejected. Alternatively, `--cpython-archive /absolute/Python-3.12.13.tar.xz`
verifies the download's size and SHA-256 before exercising the backport on
`pyexpat.c`. These fixture checks do not compile a Python distribution.

### Distribution workflow

The [PBS distribution workflow](../../.github/workflows/pbs.yml) runs for changes
to this integration directory or the workflow when the head branch starts with
`charlie/codex-oriole-pbs-`. It can also be dispatched manually with the target
and PGO options described above. The pull request path uses the generic target,
normal Rust ThinLTO build, and PBS's CPython `noopt` variant.

Validate the resulting interpreter and archive:

- Verify the installed bundle manifest, static archive hash, and parser identity.
  `pyexpat.EXPAT_VERSION` must identify Oriole.
- Run PBS's archive validator and custom checks, inspecting dynamic dependencies
  and symbol versions.
- Run the installed XML suites and threaded parsing on glibc 2.17.
- Preserve test failures, including the known callback-grouping differences in
  the [compatibility guide](../../docs/compatibility.md).

The local [CPython extension harness](../../tools/cpython/README.md) builds
separate artifacts; repeat distribution checks for each runtime and build mode.
Installed-interpreter performance also needs separate benchmarks.

macOS packaging, Windows packaging, cross builds, and fully static Python
validation remain open. Keep these experimental archives outside the release
artifact pool. The default PBS dependency remains Expat until the relevant
compatibility and release gates pass.

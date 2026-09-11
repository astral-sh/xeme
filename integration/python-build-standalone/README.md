# Experimental python-build-standalone integration

This recipe overlays Oriole's static archive and public header in the CPython build
container for [PBS revision `a4553880`](https://github.com/astral-sh/python-build-standalone/tree/a4553880293fe9d1bb62747d34ab0e5121d3554f).
It is opt-in and initially supports CPython **3.12.13**, a Linux x86_64 host,
the `x86_64-unknown-linux-gnu` target, and Docker builds of shared Python.

The ordinary Expat dependency still builds and remains in PBS's dependency cache.
The overlay replaces it only inside the selected CPython build container. A phony
Make dependency forces CPython to rebuild whenever the overlay is selected. Use a
separate PBS checkout and output directory for this experiment: resulting Python
archives retain PBS's normal filenames and must not enter the release artifact pool.

## Prepare a bundle

From a frozen Oriole checkout, with Rust 1.96 or newer and its matching documentation
component installed:

```sh
python3 integration/python-build-standalone/prepare.py --output /absolute/oriole-bundle
```

For local Ohm development, add `--toolchain ohm` and set a separate
`CARGO_TARGET_DIR` and shared `CARGO_BUILD_BUILD_DIR` as instructed by the workspace.
The output directory must not already exist. A failed build leaves its log there;
use a new output directory when retrying.

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
- `manifest.json`: source and file hashes, target, toolchain, and build command.

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

Pull requests whose head branch starts with `charlie/codex-oriole-pbs-pgo-` also
select the PGO path, allowing the unmerged integration stack to exercise it.
Other PBS pull requests keep the normal build. Manual dispatch requires the
workflow to be present on the repository's default branch.

The [recorded local PGO bundle](../../docs/validation/2026-09-11/pbs-pgo-bundle/)
passed its fresh Ohm build and both C consumers. The [normal stable PBS baseline](../../docs/validation/2026-09-11/pbs-independent-checks/)
passed the archive validator and actual glibc 2.17 threaded parsing, while retaining
the two known strict XML callback assertions. The new stable PGO distribution
trial remains pending.

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
CPython overlay; see the [full failure and correction report](../../docs/validation/2026-09-10/pbs-glibc/).


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

The runtime at [`1262888`](https://github.com/astral-sh/oriole/commit/1262888)
includes external DTD declaration grammar, internal declaration composition,
namespace and encoding corrections, foreign-DTD read policy, and completed-parser
API behavior. Its [complete distribution validation](../../docs/validation/2026-09-10/pbs-final/)
passes the archive validator, custom checks, installed XML suites, parser identity,
and the actual glibc 2.17 baseline. Source/header/manifest hashes match the
[final local validation and benchmarks](../../docs/validation/2026-09-10/final-runtime/);
the PBS stable-toolchain bundle has its own binary and distribution hashes.

The combined runtime at [`b68bdca`](https://github.com/astral-sh/oriole/commit/b68bdca)
includes the reviewed external-value continuations, declaration Default callbacks,
allocation-free shared state, and inline character data. Its
[frozen validation report](../../docs/validation/2026-09-10/external-values/)
identifies the source and local release libraries. The PBS distribution gate
rebuilds that source with its own pinned toolchain and records separate bundle and
distribution hashes. Local library results do not substitute for that full build
or its glibc 2.17 runtime check.

Check the patch and staging logic against a clean pinned PBS source tree:

```sh
python3 -m unittest discover -s integration/python-build-standalone -p test_pgo_bundle.py
python3 integration/python-build-standalone/validate.py --pbs /absolute/clean-pbs \
  --cpython /absolute/cpython-3.12.13
```

[The recorded local results](validation.json) include a real PIC archive build,
native C integration and adversarial tests linked statically, and a shared-library
link of the complete archive. Commands and source hashes are retained in the
bundle manifest. These checks passed on the development host.

This checks the C-only archive command, clean patch application, Python compilation,
shell syntax, the unchanged
default path, native-linker propagation, target restrictions, and rejection of a
mismatched header. Fixture checks do not compile a Python distribution.
With `--cpython`, validation also runs the actual backport shell block against
the pinned consumer source and verifies that reapplication is rejected.
Alternatively, `--cpython-archive /absolute/Python-3.12.13.tar.xz` verifies the
download's size and SHA-256 before exercising the backport on its `pyexpat.c`.

Before deployment, run the actual resulting interpreter's XML test suites, confirm
`pyexpat.EXPAT_VERSION` identifies Oriole, run PBS's distribution validator, and
retain archive hashes and glibc/native-library results. Re-run allocator failure,
callback lifecycle, differential, and sanitizer gates on the same Oriole source.
The CPython static-extension harness in `tools/cpython/` is a separate local gate.

The [completed Linux x86-64 validation](../../docs/validation/2026-09-10/pbs-final/)
identifies runtime `1262888`, matching the final consumer, benchmark and sustained
fuzz source. The experimental archive is retained as a seven-day CI artifact;
permanent hashes, metadata, complete compressed logs and independent audits are
in the repository. Earlier successful archives and failures remain separately
identified by their source revisions.

macOS packaging, Windows packaging, cross builds, and fully static Python
validation remain open. The default PBS dependency should remain Expat until the
relevant compatibility and release gates pass.

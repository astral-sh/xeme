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

The archive is built for the GNU target, but target compatibility remains a PBS
validation gate. At the pinned revision, PBS links x86_64 against a Debian Jessie
sysroot inside a newer container. A successful link on the development host does
not establish that older glibc baseline. Run PBS's distribution validator and inspect
its dynamic library and symbol-version results before treating the archive as portable.

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
python3 integration/python-build-standalone/validate.py --pbs /absolute/clean-pbs \
  --cpython /absolute/cpython-3.12.13
```

[The recorded local results](validation.json) include a real PIC archive build,
native C integration and adversarial tests linked statically, and a shared-library
link of the complete archive. Commands and source hashes are retained in the
bundle manifest. These checks passed on the development host.

This checks clean patch application, Python compilation, shell syntax, the unchanged
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

No complete PBS container build, older-glibc runtime check, macOS packaging,
Windows packaging, cross build, or fully static Python validation is claimed by
this recipe. The default PBS dependency should remain Expat until those relevant
release gates pass.

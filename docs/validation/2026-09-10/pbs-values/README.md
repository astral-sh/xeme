# Complete PBS build of the measured runtime

[Workflow 34444526198](https://github.com/astral-sh/oriole/actions/runs/34444526198)
passes every distribution gate at workflow source `8c60cce`. All 17 compiled
runtime Rust sources match the frozen `b68bdca` checkpoint used for the combined
benchmarks, four local CPython consumers, and three ten-minute ASan campaigns.
The build uses PBS's pinned stable toolchain, not the local Ohm compiler.

- The complete CPython 3.12.13 archive passes PBS metadata and native-symbol validation.
- Custom distribution checks report 22 tests, five skips, successfully.
- The installed interpreter passes six XML modules: 802 tests, 12 skips.
- The same artifact passes on the pinned CentOS 7/glibc 2.17 image: 1,024 threaded
  parses and all six XML modules, 802 tests with 13 skips.
- TLS fallback symbol checks and the 32-thread destructor probe pass.

Distribution SHA-256:
`d0aa33edccb236b493e779f87de83289a3dda29e736b125025b940537cb81ddd`.
The [workflow artifact](https://github.com/astral-sh/oriole/actions/runs/34444526198)
is named `oriole-pbs-experimental-distribution`, artifact ID `10140086901`.
It is retained by GitHub for seven days. Its ZIP digest differs from the contained
`.tar.zst` digest; both are retained in their respective metadata.

Full build/test logs, bundle manifests, image identity, archive hashes, GitHub
run/artifact metadata, and the independent source comparison are preserved here.
This proves the stated Linux x86-64 checkpoint. Later namespace, version, foreign
DTD, decoder, and subsequent changes require separate regression evidence; it does
not establish other distribution targets or complete Expat compatibility.

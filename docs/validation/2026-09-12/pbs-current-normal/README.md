# Current normal PBS installed validation

[PR164](https://github.com/astral-sh/oriole/pull/164),
[run 34697381695, attempt 1](https://github.com/astral-sh/oriole/actions/runs/34697381695/attempts/1)
built the selected `6320d7b75a660866cb701cf2f50a1ec61084fbf5` runtime into
CPython 3.12.13. Archive validation, custom tests, parser identity and threaded
parsing on glibc 2.17 pass. **The workflow remains failed:** both installed XML
campaigns retain `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, the
two known callback-grouping failures. No additional test failure appears in the
saved logs.

## Outcomes

| Check | Saved result |
| --- | --- |
| [Archive validator](raw/pbs-validate.log) | Archive OK |
| [Custom checks](raw/pbs-custom-tests.log) | 22 tests, 5 skips, no failures |
| [Installed XML suites](raw/pbs-xml-tests.log) | 806 executions, 4 failure executions, 12 skips; both known failures recur on retry |
| [glibc 2.17](raw/pbs-glibc217-tests.log) | `oriole_compat_2.8.4`; 1,024 threaded parses complete; XML suites then report 802 methods, 2 failures, 13 skips, child exit 2 |
| [Host TLS probe](raw/pbs-tls-probe/manifest.json) | Native, static fallback and shared fallback each check 32 thread destructors |

The main XML campaign initially runs 802 methods. Its two name-filtered retries
run two methods each, with one failure and one pass apiece; 806/four therefore
counts repeated executions, not four distinct failures. ElementTree, its C
accelerator, minidom and pulldom pass in both campaigns. The XML log also contains
a second successful invocation of the same 22 custom checks. The nonfatal
`exec_prefix` warning remains in both custom-check logs.

## Source and build identity

The generic `x86_64-unknown-linux-gnu` bundle uses stable Rust 1.98.1, normal
release ThinLTO and one codegen unit. `target_cpu` is null, PGO is false, and PBS's
CPython variant is `noopt`. The existing upstream `pyexpat.c` child-parser cleanup
backport is applied; CPython test assertions are unchanged.

The checkout merge `799eb8343e8a34c412f984310299caeffdddd0d1` and PR head
`9333ad0ea576ecfebce60a3034633ea7c32a2d1d` have the identical full Git tree
`9e9e6babb4a31be067ce2b65c1cee8be3b7afc97`.
All 76 [bundle source hashes](bundle-manifest.json) match PR-head Git blobs.
Of the selected 74-file source map, 67 are in that manifest. The seven omissions
are the CLI manifest/main, ABI README, three standalone C test programs and their
notices. All 32 Rust source files under the ABI crate and its two workspace
dependencies, all three package manifests, and the workspace manifest/lockfile
are covered. The CLI is outside this C-library build.

The downloaded archive is
`cpython-3.12.13-x86_64-unknown-linux-gnu-noopt-20260910T0100.tar.zst`, SHA-256
`5bb558748cffaf0ca4226461d93b81b232cef5d2bf6fdd48753dbf69a8e491f4`.
Its installed static archive matches the bundle:
`f3c58b8b8c988225cb0da40f4e168b40aad0ddac060e5864b5beb0e4c313d2d2`.
The [result record](result.json) retains source/build identity, raw hashes,
[run](metadata/run.json), [attempt](metadata/attempt.json),
[job](metadata/jobs.json) and [artifact](metadata/artifacts.json) metadata.

These are installed-distribution compatibility results. They establish no
installed-interpreter performance result, do not replace the separately built
local benchmark evidence, and do not establish a fully passing XML suite or
production readiness. No downloaded interpreter was executed locally during the
audit. Full build logs, TLS symbols and the experimental archive remain in the
linked CI artifacts; their hashes are retained in the result record.

# Complete PBS build of the final runtime

[Workflow 34456274540](https://github.com/astral-sh/oriole/actions/runs/34456274540) passes every distribution gate at workflow source `5f9a18f`. The build starts at 08:38:41 UTC and finishes at 09:12:40 UTC on 10 September 2026. All 26 library source/header/manifest entries match both Git runtime `1262888` and the frozen source used by the final local consumers, benchmarks and sustained sanitizer campaigns.

The PBS build uses stable Rust 1.98.1 (`48a229cea`) with PIC and unwinding enabled. Its static archive is a distinct build from the local Ohm libraries. The bundle's `libexpat.a` SHA256 is `8950524c3c576b70ac89049c71d57eadd77836b79f71f072016a26c53498c385`.

| Gate | Result |
| --- | --- |
| Complete archive | CPython 3.12.13 Linux x86-64 distribution produced |
| PBS validator | Metadata, native-library and symbol-version validation pass |
| Custom distribution checks | Successful, 22 reported tests, five skips |
| Installed XML suites | All six modules succeed, 802 reported tests, 12 skips |
| Installed parser identity | `oriole_0.0.1` |
| CentOS 7 / glibc 2.17 | 1,024 threaded parses and all six XML modules succeed; 802 reported tests, 13 skips |
| TLS destructor probes | Native, static fallback and shared fallback each execute 32 thread destructors exactly once; symbol checks pass |

The custom suite runs once directly and again before the stdlib invocation; each run has 17 non-skipped successes and five skips. The skips concern interactive curses, a newer free-threaded Python configuration, a Windows SSL regression, Tkinter GUI prerequisites and Python 3.14 zstd workers. None is an XML/Expat test. The XML suite totals in the table are separate results.

The glibc run uses the pinned CentOS image with networking disabled and the extracted distribution mounted read-only. The TLS workaround remains scoped to the pinned PBS overlay; it selects Rust's existing pthread-key fallback. This report does not establish arbitrary DSO unload safety or compatibility with other distribution targets.

## Artifact identity

The downloaded distribution is `cpython-3.12.13-x86_64-unknown-linux-gnu-noopt-20260910T0100.tar.zst` (53,133,840 bytes), with SHA256:

`decf747cc6642a11a96e372ae6a8cfaf928306aa83abf2fa9e5a64a610a898bc`

This matches the installed-interpreter job's archive hash. The archive's embedded Oriole bundle manifest matches the separately uploaded manifest byte for byte, and its combined license file matches the bundle's recorded notice hash. Both downloaded artifact ZIP digests also match GitHub's metadata:

| Artifact | ID | ZIP SHA256 |
| --- | --- | --- |
| `oriole-pbs-experimental-distribution` | 10144825762 | `6579f49753736468589693be3a5e8e980b4a968c74211cbe7d7aee2fce03aa53` |
| `oriole-pbs-validation` | 10144835861 | `19a8cd4520dd6eb13cbe20e2345e8b2f44799ba6ef5384fe98c11031b63cc4a3` |

The [distribution artifact](https://github.com/astral-sh/oriole/actions/runs/34456274540/artifacts/10144825762) is retained for seven days; validation uploads are retained for fourteen days. The ZIP and contained `.tar.zst` hashes identify different byte streams. Permanent build/test logs, metadata, source comparisons, artifact hashes and independent review are retained in this repository.

## Scope and reproduction

`evidence.tar.gz` preserves full build and validator logs, XML suite output, TLS symbols, the bundle manifest, extracted archive metadata/notices, run/artifact metadata, download verification and source comparisons. It also retains an initial metadata-audit size-limit failure: the combined license file exceeded the audit script's 1 MiB cap. The corrected 16 MiB audit rereads the same verified archive; the distribution build and runtime gates were unaffected.

Use the [opt-in recipe](../../../../integration/python-build-standalone/) with the pinned PBS revision and runtime source. This is a concrete Linux CPython integration result. The [final compatibility assessment](../final-runtime/) retains the remaining API failures, exact callback/position differences and performance gap. Broader Expat replacement and additional PBS targets remain open; no default dependency or release artifact was replaced.

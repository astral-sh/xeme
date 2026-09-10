# Version-consistent PBS distribution validation

The [full PBS workflow](https://github.com/astral-sh/oriole/actions/runs/34516979948)
on `4b11ace3d89fe6b7bf66ca55290eb23444f11de2` **failed** on two unchanged
CPython XML assertions. The distribution build and archive validator succeeded.
Both custom-suite invocations reported 22 tests with five skips and 17 successes.
The installed-identity and glibc 2.17 CI steps were skipped after the XML failure.

| Check | Outcome |
| --- | --- |
| Complete CPython 3.12.13 distribution build | Passed |
| Distribution structure validator | Passed |
| CI XML suite | Failed: two distinct methods; 802 initial executions plus four reruns |
| Local installed identity check | Passed: built-in `pyexpat` and `_elementtree` use Oriole |
| Local original glibc 2.17 gate | Failed: 1,024 threaded parses passed, then the same two XML assertions failed |
| Local verbose XML suites, host and glibc 2.17 | Each reports 802 tests, two failures, 13 skips and three expected failures |

The failures are `test.test_pyexpat.BufferTextTest.test1` and
`test.test_sax.CDATAHandlerTest.test_handlers`. Both concern ordinary character-data
callback boundaries. All 802 recorded method outcomes agree between the two local
verbose runs. No assertion, test source or test bound was changed.

## Distribution identity

The downloaded archive contains 6,546 members and is 53,609,802 bytes. Its SHA-256 is
`504b3457b6ed8205f9f884bd1c454504e041982f55ce24e85aab20ca7793050e`.
This hash was computed locally because CI skipped the later archive-hash step.
All 64 bundle source hashes match Git `4b11ace`; the archive's `libexpat.a` matches
the CI bundle hash `85f657e19a9f821703f5cd4b809aab7b9751ef571c41968593effebbc7589747`.

The installed built-ins report `oriole_compat_2.8.4` and `(2, 8, 4)`.
The distribution uses stable Rust 1.98.1 and its target sysroot. Its binaries
differ from the local Ohm and profile-guided libraries; the shared identity is
the source revision. [report.json](report.json) retains every native artifact hash.
The bundle applies the pinned [CPython allocation-failure cleanup backport](../../../../integration/python-build-standalone/consumer-fix/)
to `Modules/pyexpat.c`; the upstream XML tests remain unchanged.

The local glibc check used the original script, a pinned CentOS image, networking
disabled and read-only mounts. Its container/script exited 1 after the XML child
exited 2. The recorded dynamic symbols require no GLIBC version newer than 2.17;
actual loading and the 1,024 successful threaded parses provide separate evidence.
**The workflow and local glibc gate remain failed.** These local follow-ups do not
change the status of the skipped CI steps.

## Test counts and earlier local coverage

CI ran 802 methods initially. Its two broad rerun filters selected four methods,
producing 806 total executions and four failure occurrences from the same two
failing methods. CI enables the CPU resource, accounting for 12 skips there
versus 13 in the default-resource local runs.

Earlier local harness runs reported 803 tests and 31 skips. The harness loaded
its two intended extension modules at startup, but CPython's fresh-import tests
then selected the managed interpreter's incompatible built-in `_elementtree`.
That caused 19 accelerator-specific skips. The PBS build executes 18 of those
successfully; one still skips because it requires 2 GiB of memory.

With a working accelerator, the pure-Python `NoAcceleratorTest` skips during
class setup, with no method counted. That explains 802 rather than 803. The
availability of `_testcapi` and genuine C-versus-Python behavior explain other
status differences. The [detailed report](report.json) preserves all changed
statuses and the exact initial/fresh module probes. Earlier raw results remain
unchanged and must be read with this coverage limitation. The persistent import
routing correction is documented in [the harness guide](../../../../tools/cpython/).

## Evidence and review

[evidence.tar.gz](evidence.tar.gz) retains the immutable handoff, downloaded CI
logs and bundle manifests, exact bundle sources, unchanged CPython/PBS test
wrappers, local commands and logs, module probes, analysis scripts and root's
independent saved-evidence review. Its SHA-256 is
`f4b2649d58e349d28db02fdfd4faa87afc255d65f9e5ef6feb24863bca6d6063`.
[files.json](files.json) records its 12 outer members. The embedded manifests
record 114 regular members in three nested archives, all rehashed during root
packaging. The interpreter, libraries and downloaded distribution are excluded;
their identities are retained.

The initial count-analysis script failed its own assertion because it did not
handle multiline unittest headers. Both analysis versions remain available;
correcting that analysis required no test rerun. The [root review](root-review.json)
independently checks source and binary hashes, raw module totals and failure
names, fresh-import probes and the threaded result. It does not claim an
independent compiler/container rerun or full per-method reconciliation.

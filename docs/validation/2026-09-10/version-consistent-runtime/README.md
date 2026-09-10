# Version-consistent runtime validation

This report identifies runtime commit `4b11ace3d89fe6b7bf66ca55290eb23444f11de2`.
It records normal and profile-guided builds from the same frozen source. The
complete source/tool snapshot contains 358 files; the runtime inventory contains
64 files. Original reports, logs, source, commands, upstream test bodies and
independent reviews are preserved in `evidence.tar.gz`, with member hashes in
`files.json`. Compiled binaries are omitted and identified by hash.

| Build | Shared library SHA-256 | Static archive SHA-256 |
| --- | --- | --- |
| Normal | `ac6a0a6adcabba65ef338d362f438c588ad4a01a9821989bae28f445ba7778ac` | `7fd04b4de8bf7aa2cc490d35ece6f6c7a45d4106f4247f05d501e793015e43e6` |
| Profile-guided | `4584afbf7361732cf506f762cf25885cded19ecc36ee6b03053ae674bf809505` | `66834d59ce8f97a14c6164e832a1b94bcb9293462fe7c019ca01b3d38da700c4` |

## Version consistency

`XML_ExpatVersion()` returns `oriole_compat_2.8.4`, matching the numeric API target
from `XML_ExpatVersionInfo()` and the header. `ORIOLE_VERSION` and the Cargo package
retain implementation version `0.0.1`. A compatibility target does not establish
complete behavioral or security equivalence with Expat.

The new C regression fails against the preceding library and passes against both
the final library and Expat 2.8.4. The twelve original version configurations still
fail Expat's literal identity assertion; they now pass the previously inconsistent
numeric comparison. No change in the passing configuration count is claimed.

## Why 417 API configurations fail

The original matrix contains 395 public test names, six chunk widths and two
deferral settings: 4,740 configurations. The current result is **4,323 passing /
417 failing**, spanning 40 failing test names. These are not 417 separate missing
features. Against the original 987 failures, 584 configurations are fixed, 403
remain failing and 14 newly fail allocation-schedule assertions.

| First failing assertion | Configurations | Interpretation |
| --- | ---: | --- |
| Allocation retry ceiling | 298 | Real allocation-cost gaps: parsing still exceeds the test's bounded retry allowance. |
| Fixed allocation-failure stage | 12 | The supplied budget fails during child creation before the nested parse stage expected by the test. |
| Expected reallocation schedule | 44 | Successful parsing does not perform the specific reallocation the assertion expects. |
| Expected malloc after empty `XML_ParseBuffer` | 12 | Both reject parsing before `XML_GetBuffer`; afterward, Oriole succeeds without the malloc that Expat attempts. |
| Literal version identity | 12 | The compatibility revision agrees, but the implementation identifies itself as Oriole. |
| Error column | 12 | Exact diagnostic positions differ. |
| Deferred child callback timing | 12 | Exact callback timing differs under reparse deferral. |
| Allocation-byte heuristic | 1 | A retained allocation-cost gap; the observed byte delta does not by itself establish quadratic behavior or harmlessness. |
| One-GiB buffer request | 12 | The request cannot fit within the harness's one-GiB process address-space limit, including reference runtime overhead; Oriole also has explicit limits. |
| Stream exceeding two GiB | 2 | Oriole reaches its input limit; the reference also fails under the unchanged three-second test timeout. |
| **Total** | **417** | **All original assertions remain failed.** |

The [complete classification](remaining-api-classification.json) retains source
bodies and each first-failure observation. Later assertions in a failing test are
not automatically established. No larger retry ceiling, artificial allocation,
altered resource limit, or diagnostic experiment counts as an original pass.

## Rust, C and CPython

The workspace passes 368 tests plus one compile-fail documentation test, strict
Clippy and formatting. The supplementary root check report retains the original
source manifest unchanged and identifies the documentation test's captured tool
output; no separately redirected documentation log is claimed.

Both builds pass six shared/static native C processes, including 327 selected
allocation scenarios per linkage. Those C consumers use ASan/UBSan; release Rust
is uninstrumented and LeakSanitizer is disabled. Selected allocation ownership is
checked independently. Sustained Rust sanitizer campaigns identify their own
binaries and results separately.

Each build runs four CPython configurations: shared/static linking, each with the
original consumer and the cleanup backport. Every configuration reports **803
tests, 31 existing skips and two strict failures**, exiting with status 2:

- `test.test_pyexpat.BufferTextTest.test1`
- `test.test_sax.CDATAHandlerTest.test_handlers`

Both concern text callback boundaries. Separate semantic checks pass 2/2 in every
configuration; they do not turn the unchanged upstream suite green. The 3,212
named test records and 32 subtest records agree between normal and optimized
builds. Module origins, compatibility versions and library identities are checked.

## W3C acceptance

Each build and reference records **4,962 mandatory passes, 960 mandatory failures
and 81 optional observations** across 2,001 descriptors and three chunk widths.
Current C acceptance agrees with Expat. Of the failures, 954 observations are
Fifth Edition name fixtures rejected under the C interface's deliberate Fourth
Edition policy. The historical 5,916/6 result belongs to the earlier policy.
This is neither a full safe-Rust conformance claim nor a canonical-output test.
The pinned mirror is preserved; its identity against the official archive remains
unverified.

The normal collection controller mistakenly invoked W3C twice. The second worker
hit the existing-output guard before parsing. The first complete parser rows,
summary, catalog and worker logs remain intact and count once; only the outer
wrapper log and command metadata were overwritten. The wrapper ignored that
second worker's failure. The original controller, per-launch metadata and initial
incorrect interpretation remain available. Independent review traced the guard
and verified all before/after/current hashes. The optimized scan ran once with the
corrected controller.

## Scope

The [preceding consumer evidence](../prior-buffer-consumers/) preserves the earlier
normal and optimized builds unchanged. Benchmarks, sustained fuzzing and the full
PBS distribution gate have separate source/build identities and completion
reports. These bounded results do not establish a general production replacement,
and no faster-than-Expat claim follows from these correctness checks.

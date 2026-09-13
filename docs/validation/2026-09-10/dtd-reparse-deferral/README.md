# DTD reparse deferral

The existing reparse-deferral setting now also guards ordinary DTD token scans.
An incomplete nonfinal declaration waits for the existing growth threshold.
Final input, decoding errors and token-limit overflow force processing; active
ATTLIST continuations retain their earlier dispatch precedence. Internal
replacement sources remain final and bypass this guard.

| Original upstream API matrix | Passing | Failing |
| --- | ---: | ---: |
| After the entity error-origin fix | 4,335 | 405 |
| Both fixes combined | 4,347 | 393 |

All 12 configurations of `test_reparse_deferral_is_inherited` now pass, with no
other outcome change. The 395 original test names, six chunk widths, two deferral
settings, assertions and bounds remain unchanged. The runner still exits 1 for
the remaining failures. [api-comparison.json](api-comparison.json) preserves each
changed row. The [earlier failure classification](../version-consistent-runtime/)
still applies after removing its error-column and deferred-child timing rows.

## Validation and retained differences

The combined source passes 373 workspace tests, strict Clippy and formatting.
Another 3,318 strict observations match the entity-fix runtime exactly, including
callbacks and final positions. That comparison uses Oriole as its baseline;
it does not assert complete parity with Expat.

The isolated DTD patch independently passes six shared/static native C sanitizer
processes, including 327 selected-allocation scenarios per linkage. Those C
consumers use ASan/UBSan; release Rust is uninstrumented and leak checking is
disabled. The isolated patch's 372 workspace tests and original API result
(4,335/405) exclude the earlier entity-origin fix. The [summary](summary.json)
keeps these source scopes separate.

The focused Expat oracle covers 54 configuration pairs: inherited/overridden
settings, UTF-8 and both UTF-16 orders, growth, empty feeds, disabling and final
input. All 18 incorrect token-completion callback counts are corrected. Nine
later growth-stage comparisons still release one 100-space feed earlier than
Expat. These differences remain recorded; exact feed timing is not claimed.

Three existing Rust tests assumed immediate parsing without final input. The
accounting test now explicitly disables deferral before sampling committed work;
the error/budget tests finalize their input. Their work, position, error and
callback-count assertions and limits remain unchanged. New boundary tests cover
token-limit overflow, decoder errors and active ATTLIST ordering.

## Source and evidence

The combined source is based on `4596510` (PR101). Its 65 recorded source files
match the isolated final DTD source except for the two previously reviewed
entity-origin files. The test-overlay release rebuild is byte-identical to the
library used by the combined API and differential checks. A later test-only
Clippy cleanup replaces three temporary vectors with fixed arrays.

Both initial root test launches failed before execution with OS error 30. The
final checks use the same Ohm compiler with experimental defaults disabled and
pass. An earlier attribution to an accounting-test failure was an incorrect
inference; both root logs contain only the tooling error. The actual three
test-precondition failures belong to the isolated author's preserved logs.

[evidence.tar.gz](evidence.tar.gz) retains the isolated handoff, combined build
attempts and source phases, original/candidate API data, strict traces, test logs
and review scripts. Its SHA-256 is
`18f16a4f3aeddf8f11b5e5fe7efc3d954022d9c45527cfa034dd0bf0418cd7a9`.
[files.json](files.json) records 189 outer members; the isolated nested archive
has 188 members. Packaging read back every outer member. Executables and
libraries are excluded and identified by hash.

The [API review](api-review.json) independently reconstructs the complete outcome
and trace comparisons. The [source/check review](source-review.json) verifies
final source identities, all 373 successes, compiler-check exits and the corrected
failure attribution. Neither review claims another build or parser run. Earlier
benchmarks, PBS and sustained-fuzz reports retain their original runtime identities.

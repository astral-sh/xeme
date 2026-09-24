# Expat 2.8.5 follow-up cleanup

These changes follow the [2.8.5 compatibility update](2026-09-24-expat-2.8.5.md).

## API failure baseline

The API baseline now allows 495 failing configurations, down from 509. All twelve
configurations of `test_nsalloc_long_element` and the two whole-buffer
configurations of `test_alloc_realloc_subst_public_entity_value` must pass.
The remaining allowances and their assertions are unchanged.

Both recorded 2.8.5 API matrices pass the tighter gate with 495 known failures
and no remaining improvements. The earlier 2.8.4 run also passed these fourteen
configurations; this cleanup records an existing improvement.

For each removed allowance, a validation probe changed the recorded successful
row back into its former assertion failure, including the log and result counts.
The old baseline accepted all fourteen injected regressions. The new baseline
rejected every one. The local report is
`.cache/review-stack/cleanup-baseline-check.json`.

## Loaded reference version

All four pinned gates now require the reference worker to report `expat_2.8.5`.
The upstream API/allocation bridge records `XML_ExpatVersion()` beside its loaded
library origin; the W3C and differential workers already report it. `gate.json`
retains both engine versions alongside the selected library hashes.

A harness test covers six reports for each of the four gates: 2.8.5 succeeds;
2.8.4, a future version, a candidate version, null, and a missing version fail,
even when semantic checks succeed. The existing API-limit test also now resolves
its temporary paths, matching the runner on macOS.

All four full Linux suites passed with the tightened baseline and version check:

| Gate | Result |
| --- | --- |
| API | 4,776 configurations per engine; candidate has 495 known failures and no remaining improvements; reference passes every configuration |
| Allocation | 1,008 configurations and ownership reports per engine, all passing |
| W3C C interface | 6,003 rows per engine; the same 960 disclosed conformance failures in each; no semantic differences |
| Differential | Named fixtures and 200 generated cases pass |

Every report identifies `xeme_compat_2.8.5` and `expat_2.8.5`. The 22 harness
self-tests pass on Linux; macOS passes 20 with the two Linux-only isolation tests
skipped. Ruff and ty (Linux target) pass. Raw results are under
`.cache/review-stack/expat-285-followup-linux-*`.

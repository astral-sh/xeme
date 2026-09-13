# Buffered-parser continuation coverage

All **12 diagnostic configurations pass** on the selected reference-frame PGO library. The original `test_nsalloc_parse_buffer` stopped at an allocator-specific expectation before reaching its suspension and resumption checks. Recording that one call's result allowed its unchanged continuation to complete.

The original API matrix remains **4,347 passes, 391 assertion failures and two timeouts across 4,740 configurations**. All 12 corresponding original configurations still fail. This separate copied consumer changes no runtime, original harness, accepted-failure list or retry ceiling.

## What the result establishes

The original test reserves a buffer, enables allocation failure, then expects an empty `XML_ParseBuffer` call to fail with `NO_MEMORY`. In every diagnostic context, that call instead returned `OK` with error `NONE`; the allocation counter remained zero. The diagnostic records these values without requiring an allocation or interpreting the two outcomes as equivalent.

Every parser API call is retained. Only the two assertions for that second empty call become an observation, with an explicit stdio include for logging. Its read-only error query now also runs on the success path that previously stopped at an assertion. All subsequent source and assertions are unchanged:

| Stage | Existing assertion reached |
| --- | --- |
| Before input | A fresh unreserved buffer parse reports `NO_BUFFER`; resuming an unsuspended parser reports `NOT_SUSPENDED`. |
| Character callback | The callback stops the parser resumably and clears itself; parsing returns `SUSPENDED` with no error. |
| Suspended parser | Further ParseBuffer/GetBuffer calls are rejected. |
| Resume and finish | Resumption succeeds; subsequent ParseBuffer/GetBuffer calls are rejected as finished. Fixture teardown completes. |

This verifies the exact continuation after the observed empty call. It does not establish the removed allocation/OOM expectation, allocator call counts, exhaustive OOM behavior, or callback text/count oracles that this fixture never contained.

## Execution and review

The root agent compiled the copied C consumer once and ran one test across chunks 0–5 and both deferral modes. Both commands exited zero. The chunk setting does not affect this particular body; all 12 original configurations were retained. Original limits remained 3 seconds per context, 1 GiB address space, sampled RSS limit 768 MiB, and 240 seconds overall; compilation had a separate 120-second bound. Neither the C consumer nor Rust PGO library was sanitizer-instrumented.

The first preparation had a missing stdio include because its insertion anchor was absent. Source readback caught it before any target ran. The corrected include, original preparation script, first receipt and both patches are archived. No parser calls or tail assertions changed during that correction.

Selected library SHA-256: `d3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4`. Measured source: `0f66d54ac8418f0a6e628ad18570677a19c9ed45`, published in [PR #144](https://github.com/astral-sh/oriole/pull/144) at `4064b0653534aff690c0e0f395a6b3b48da878fc`.

The saved-output reviewer prepared the diagnostic but did not execute it. The readback reconstructed all 12 observations and passes and all 4,740 original outcomes from raw logs, reconstructed the source patch, and checked the complete unchanged tail, 33 other adapted sources, 78 original-file hashes, 86 staged-file hashes, compiler, compiled consumer and both selected-library copies. This is a review separate from execution, not an independent author review.

## Evidence and remaining scope

[Assertion reachability](assertion-reachability.md) summarizes the 34 nonpassing allocation-related test names. This diagnostic closes the exact buffer-continuation gap found there. The separate six-case replay already covers every post-loop text/handler-flag assertion among those names; neither result changes original failures or establishes general production readiness.

[report.json](report.json) preserves the execution report; [review.json](review.json) records the saved readback. `records.tar.gz` contains 107 source/log/manifest records, including the first preparation correction, original matrix, diagnostic observations, prior six-case evidence and detailed reachability audit. It excludes executables and libraries. `records.files.json` indexes archive members; `files.json` indexes the top-level packet.

Run `python3 review.py` for a portable saved-record check. `--local /tmp/oriole-nsalloc-buffer-tail-diagnostic` additionally rehashes preserved local inputs and binaries. Neither mode loads a parser or executes targets. Archived execution scripts retain original absolute paths and are provenance, not a portable build recipe.

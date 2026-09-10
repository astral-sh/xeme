# Custom encoding on the optimized lifecycle baseline

This checkpoint composes [the frozen provenance layer](../custom-encoding-provenance/)
onto `dc552f4`, retaining token/attribute recycling, offset-based attributes,
optional expanded element names, boxed declaration events, diagnostic fixes,
randomized hash salts and sticky entity-value state. It precedes the later token
ownership optimization. The shared library SHA-256 is
`11fffafee50dfef91328cb142e7d50a38578c138333860d6327892a61d09214b`.

[The report](report.json), [evidence](evidence.tar.gz), [file manifest](files.json)
and [checksums](SHA256SUMS) preserve this exact composition separately from the
original implementation evidence.

## Compatibility and safety

All 4,740 upstream configurations complete: **4,051 pass and 689 fail**. Exactly
48 formerly failing configurations now pass; none become failures. Adapted source
hashes and assertions are unchanged. One existing 2 GiB-input failure changes from
assertion exit 100 to the runner's three-second alarm. A focused replay repeats
the alarm; allowing ten seconds reaches the original assertion because the parser
retains its absolute input ceiling. The original failing outcome remains in the
full matrix. This is a runtime/timing regression on an already unsupported large
input, not an additional pass.

All 3,918 ordinary baseline/candidate differential cases match exactly. The
114,714 converter configurations repeat the original acceptance results, including
zero acceptance differences and the documented 5,112 successful external default
callback differences. Rejected-input callbacks, errors and positions remain in
the evidence. Custom encoding and unrelated remaining API failures are not waived.

The affected packages pass 262 tests: the full 261-test composition plus one
additional attribute identity regression, followed by the complete 12-test
multibyte suite. Strict Clippy passes. Six native shared/static ASan/UBSan suites
pass, with 341 allocation-failure scenarios per linkage. Rust release code is
uninstrumented and leak sanitizer is disabled. A separate bounded source review
finds no issue in metadata reuse, attribute offset/decoded-name lifetimes,
recycling, or optional expanded-name ownership; no independent test execution is
claimed by that review.

## Performance

The same five-pair project screen on CPU 1 reports candidate/control median
throughput ratios of **0.882–0.981 at 4 KiB** and **0.879–0.913 at 64 KiB**. Most
ordinary project cases cost roughly 9–12% on this optimized baseline. All callback
hashes and counts agree. These shared-host native-driver observations are not
actual project process timings or comparisons against Expat. This overhead needs
further profiling and optimization before a final readiness claim.

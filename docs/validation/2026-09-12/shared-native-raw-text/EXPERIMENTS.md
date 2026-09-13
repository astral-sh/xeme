# SIMD, arenas and allocator experiments

The selected parser already uses 16-lane text classification, warmed literal-attribute spans, reusable name owners, and effective ThinLTO. The recent evidence favors removing duplicate input/raw-text storage; wider vectors or fewer allocations alone have not reliably improved elapsed time. The bounded current-source ASan/fuzz campaign has passed; raw-view remains the intended publication candidate.

## Current ordinary-build experiments

Every measured row below compares with the same selected source/runtime `6320d7b7` (PR165 base `2c4d216`, normal library `ce0a648c`). Ratios are **candidate time / selected time**; lower is faster. Native real: 24 conditions; generated: four; Python: 24. Each uses the original seven-pair method. `first → confirmation` keeps epochs separate; `—` means no completed measurement in scope.

| Experiment | Status | Native real | Generated | Python |
| --- | --- | ---: | ---: | ---: |
| Selected 16-lane/warmed-frame implementation | Selected reference | 1.0000 | 1.0000 | 1.0000 |
| 32-lane TextPlan SIMD | Rejected | 0.9916 | 0.9979 | 1.0061 |
| Eight returned name owners (was two) | Rejected | 0.9927 | 1.0454 | — |
| Amortized arena attribute-record growth | Rejected | 1.0157 | 1.0502 | — |
| 22-byte inline stack names, checked string projection | Not selected | 1.0003 | 1.0247 | — |
| Scalar planned-name table / sticky ASCII proof | Measured, not selected | 0.9848 → 0.9847 | 1.0235 → 1.0031 | 0.9937 → 0.9978 |
| Shared native input + raw Text view | Intended; ASan passed | 0.9697 → 0.9776 | 0.9845 → 0.9935 | 0.9824 → 0.9838 |
| Byte-based inline-name projection | Rejected | 0.9780 | 1.0234 | 1.0043 |
| General Start tail in a separate non-inlined helper | Rejected | 0.9866 | 1.0440 | 0.9973 |

Text32 improved the native aggregate by 0.84% but regressed Python by 0.61% (16/24 adverse conditions). Eight cached names gave a small real-file gain but regressed all four generated conditions; record growth regressed 20/24 real conditions and all generated conditions. Initial inline names were approximately unchanged overall, with namespace-enabled and generated regressions. Scalar-name improved native time in both epochs; its Python gains were smaller, and it remains unselected. These are recorded selection outcomes, not significance claims.

Byte-inline now shows a 2.20% real-file native gain (20/24 wins), with a 2.34% generated regression. General-tail extraction shows a 1.34% real-file gain (22/24 wins), with a 4.40% generated regression; its entity fixtures regress 7.52–8.50%. Both regress all four generated conditions. Byte-inline is rejected: Python is 1.0043× selected, with 15/24 adverse conditions (ElementTree 0.9990×; pyexpat 1.0095×). General-tail is also rejected: its 0.27% Python gain (0.9973×, nine adverse conditions; ElementTree 0.9983× and pyexpat 0.9964×) does not justify the entity regressions. Neither is composed into raw-view.

Raw-view reduced native real-file time by 2.2–3.0% and Python time by 1.6–1.8% across its separate epochs. In confirmation it takes **1.2659× Expat natively and 1.0975× through Python**; ElementTree alone remains 1.1423× and pyexpat events 1.0544×. Thus the Python aggregate meets the project performance target of approximately 1.10× Expat’s time, while native and ElementTree do not. All four adverse conditions and the API/strict/W3C limitations remain in the [current evidence notes](README.md).

Raw-view alone is the intended publication candidate; the [bounded current-source ASan/fuzz campaign](qualification/asan/) has passed. The later [source commit binding](source/published-source-binding.json) records `67c704c123b8661a3ad1f91bd48c2f0be4996096` with the tested bytes unchanged.

## Compiler and allocator evidence already retained

These historical comparisons have their own source/control identities and are not additional gains to multiply into the current table.

| Experiment | Status | Native real / its control | Generated / its control | Python / its control |
| --- | --- | ---: | ---: | ---: |
| Effective C-library ThinLTO, same old runtime source | Selected build method | 0.9819 | 0.9799 | 0.9890 |
| Explicit jemalloc C memory suite | Default unchanged | 0.9987 | 0.8851 | — |
| Explicit mimalloc C memory suite | Default unchanged | 0.9998 | 0.8772 | — |

The [ThinLTO native audit](experiments/historical-thinlto/native-groups.json) and [Python audit](experiments/historical-thinlto/python-review.json) compare fresh normal `9815cc85` with effective C-only ThinLTO `a55ca0f0`, with identical runtime source. Current ordinary builds already use `-O3`, ThinLTO and one codegen unit.

The [allocator report](experiments/historical-allocators/RESULTS.md) used **older, already-built PGO libraries**, not the current normal parser; this note performs no PGO work. Real-file time changed by less than 0.2%, while workload peak-RSS ratios rose to 1.0301 with jemalloc and 1.7340 with mimalloc. Generated improvements, every adverse result, provider startup memory and shared-host overlap remain explicit. This was a native C memory-suite experiment, not process-wide or CPython allocator evidence.

## Evidence and interpretation

All condition rows remain in the linked saved readbacks; the summary does not discard adverse cases. Allocation counts and code size informed source hypotheses but are not elapsed-time gains or proof of their causes. Host affinity is not isolation, and separate candidate runs are not head-to-head measurements. No target was run to prepare this note.

- 32-lane TextPlan SIMD: [first native](experiments/text-32/study/native-review.json), [first Python](experiments/text-32/study/python-normal-review.json).
- Eight returned name owners (was two): [first native](experiments/eight-name-cache/study/native-review.json).
- Amortized arena attribute-record growth: [first native](experiments/amortized-frame-records/study/native-review.json).
- 22-byte inline stack names, checked string projection: [first native](experiments/inline-stack-name/study/native-review.json).
- Scalar planned-name table / sticky ASCII proof: [first native](experiments/planned-scalar-name/study/native-review.json), [first Python](experiments/planned-scalar-name/study/python-normal-review.json), [confirm native](experiments/planned-scalar-name/confirmation/native-review.json), [confirm Python](experiments/planned-scalar-name/confirmation/python-normal-review.json).
- Shared native input + raw Text view: [first native](measurements/initial/native/review.json), [first Python](measurements/initial/python/normal-review.json), [confirm native](measurements/confirmation/native/review.json), [confirm Python](measurements/confirmation/python/normal-review.json).
- Byte-inline: [native readback](experiments/inline-stack-name-bytes/study/native-review.json), [Python readback](experiments/inline-stack-name-bytes/study/python-normal-review.json), [final decision](experiments/inline-stack-name-bytes/final-decision.json).
- General Start-tail extraction: [native readback](experiments/general-start-tail/study/native-review.json), [Python readback](experiments/general-start-tail/study/python-normal-review.json), [final decision](experiments/general-start-tail/final-decision.json).

# Share native input and raw Text storage

Validated shared native/raw Text source: `67c704c123b8661a3ad1f91bd48c2f0be4996096`. This source commit was created after the measurements and qualification; the [source binding](source/published-source-binding.json) verifies that all 330 source, auxiliary, harness and seed files match the tested snapshot. Current-source API, C, strict CPython and W3C records have completed; bounded Rust ASan/fuzz qualification has completed.

For native UTF-8 C input, we use the parser's root input buffer for both decoded input and Expat input context. A prepared native Text callback can also keep an absolute range into that buffer instead of copying the same text into `current_raw`. The range participates in input retention and is resolved through a checked UTF-8 slice. Encoding conversion, partial or invalid UTF-8, custom encodings and explicit raw overrides retain their owned paths.

The change removes duplicate storage and copying on eligible native paths. The core remains safe Rust; allocator routing, structural and live-memory limits, dependencies and compiler options are unchanged. Allocation schedules and retained-buffer capacity can change. The C adapter retains its existing callback reentry protections and frame reservation. The raw Text range is bounded by the existing 4 KiB eligibility, although it can extend input-window retention until a later raw token replaces it. No PGO is used.

## Ordinary-build measurements

The campaigns use a shared Linux x86-64 host, with elapsed work pinned to CPU0. Oriole is an ordinary generic x86-64 build with Ohm Rust 1.98.1-dev, O3, ThinLTO and one codegen unit. Expat 2.8.4 uses GCC 13.3, O3 and a shared library without LTO. Python consumers use unmodified CPython 3.12.13 pyexpat and ElementTree sources compiled at O2. These compiler recipes are held fixed across the candidate and selected Oriole builds; no PGO or CPU-specific tuning is used.

Each epoch uses six pinned real-project XML files, 4 KiB and 64 KiB feeds, and the existing seven-pair process protocol on the shared Linux host. Native results include namespaces off/on; Python results include ElementTree and pyexpat event callbacks. These are XML workloads from projects, not whole-project execution times. Each condition uses the median of seven paired process-median ratios; table entries are equally weighted geometric means of those condition ratios. Lower ratios are faster.

| Workload | First run / selected | Confirmation / selected | First run / Expat | Confirmation / Expat |
| --- | ---: | ---: | ---: | ---: |
| Native, 24 real-file conditions | 0.9697 | 0.9776 | 1.2649 | 1.2659 |
| Python, 24 real-file conditions | 0.9824 | 0.9838 | 1.0948 | 1.0975 |
| ElementTree, 12 conditions | 0.9823 | 0.9813 | 1.1342 | 1.1423 |
| pyexpat events, 12 conditions | 0.9825 | 0.9863 | 1.0569 | 1.0544 |
| Native, four generated conditions | 0.9845 | 0.9935 | 3.3812 | 3.4111 |

The candidate takes 2.2–3.0% less time than selected in the two native real-file epochs and 1.6–1.8% less in Python. Both Python aggregates fall within the project performance target of approximately 1.10× Expat’s time. The native aggregate and ElementTree subgroup remain outside it. In each Python epoch, nine of 24 conditions are within 10% of Expat; the aggregate does not establish per-input parity.

All four adverse conditions remain visible: initial Python Vulkan, 4 KiB, ElementTree (1.0018× selected); native confirmation Maven, 4 KiB, namespaces on (1.0010×) and generated rare declarations, 4 KiB (1.0124×); Python confirmation DocBook, 64 KiB, pyexpat events (1.0026×). The initial native run improved all 28 conditions. Across the four separate epochs, 104 condition rows cover 2,496 workers and 265,080 samples. Epoch ratios are not pooled, and these observations do not establish statistical significance. CPU affinity is not exclusive host isolation; host observations are retained.

## Validation before publication

The successful ordinary build passes 475 tests in 35 groups, formatting, workspace and in-process benchmark Clippy. Two earlier attempts failed in new test fixtures; their logs and exact test-only corrections are preserved. Production code was unchanged by those corrections.

The completed current-source upstream API matrix has 4,349 passes, 391 failures and no timeouts across all 4,740 original configurations. Every ordered outcome matches selected, including both passing 2 GiB input cases. All six C consumers pass.

Strict CPython covers 802 methods and 809 rendered outcomes per shared/static linkage, preserving every selected outcome and raw exit 2. The two failures remain `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, both callback text-grouping expectations. Two separate semantic checks pass per linkage; they do not make the strict suites green. Strict consumers include the pinned upstream pyexpat cleanup backport and are distinct from the unmodified benchmark consumers.

Separate allocation diagnostics and their saved readbacks completed with 300 retry passes and 12 buffer-tail passes. The retry raises only 25 per-test local allocation ceilings to 512, retaining semantic assertions. The buffer diagnostic replaces exactly two assertions with observations so remaining tail assertions run. These supplemental results do not relabel the original 391 API failures. Allocation schedule differences require assertion-level review; fewer allocation calls do not establish upstream conformance.

The W3C acceptance comparison covers 2,001 descriptors at three chunk sizes (6,003 rows per engine), with no acceptance differences, worker failures or inconclusive cases. The raw conformance run exits 1: both engines record 4,962 mandatory passes, 960 mandatory mismatches and 81 optional observations. Of the mismatches, 954 rows are explicitly Fifth Edition name fixtures; both C interfaces deliberately use Fourth Edition name rules. Six other shared mismatches concern external-entity version and BOM/encoding declarations. No failures were waived or relabeled. Error-code, position and child-outcome differences remain recorded, so acceptance agreement does not establish exact API or callback equivalence. The [full classification](qualification/W3C-classification.md) preserves these mismatches.

Six current-source Rust ASan/fuzz harnesses pass: 69,199 replay executions and 7,978,060 exploration executions, with no recorded failure artifacts. Nine required Rust crate/harness invocations are instrumented; all 22 commands exited zero and were reaped. Leak detection is disabled. The [ASan report](qualification/asan/) retains exact bounds, command logs and limitations; this is not exhaustive safety or installed-distribution proof.

## Identity

- Candidate source: [source map](source/source.json) and [exact patch](source/candidate.patch), a nine-file tested snapshot recorded subsequently as commit `67c704c123b8661a3ad1f91bd48c2f0be4996096`, based on PR165 `2c4d21647efdefe0b7da977ca206b8552c32f0f3`.
- Independent [source review](source/independent-source-review.json) covers the raw-view proposal and its input-owner composition; the ordinary-build records separately retain later formatting and test-fixture corrections.
- Candidate shared library: `e59d89d6e21b92042b8358bd8ceef45b0a60404c61c9aa06ccf2e85a00f1c7f6`.
- Candidate static library: `4e0e5f21d37797429654a15a79e1b44960614b93b1548635be6247c79b6c4105`.
- Selected control shared library: `ce0a648ccd0c661e88e9c29643bb1d3893abfac3976ebc603b28f40eb0623229`.
- Initial measurements: [native](measurements/initial/native/review.json) and [Python](measurements/initial/python/normal-review.json).
- Confirmation measurements: [native](measurements/confirmation/native/review.json) and [Python](measurements/confirmation/python/normal-review.json).
- Current-source qualification: [API/C](qualification/api-c-readback.json), [strict/semantics](qualification/strict-semantic-readback.json), and [W3C](qualification/w3c-saved-readback.json).

This snapshot includes the single-input-owner change and the raw Text view together. It does not include the scalar-name, eight-name cache, record-growth or inline-stack-name experiments.

## Related experiments and retained raw evidence

The [experiment summary](EXPERIMENTS.md) records the wider SIMD, arena, name-cache, inline-name, scalar-name and outlining results, including rejected candidates. The six root-README timings are reconstructed in [this table record](measurements/readme-benchmarks.json), with an [independent raw-worker review](measurements/readme-table-independent-review.json).

[Full raw artifacts](LOCAL_RAW_ARTIFACTS.md) remain at explicitly local retained paths. These are not public download URLs, and the compact report does not contain every worker output. [The copy index](COPY_INDEX.json) records the exact source and staged-file hashes.

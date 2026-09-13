# Final runtime validation

This checkpoint freezes the parser at PR59 (`1262888`) and the fuzz harness at PR61 (`338dcd0`). The shared library SHA256 is `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`; the static archive SHA256 is `4fa096d9ffdfcf2c01d764ae83190ab689064e10a87860822d6301d04da62651`. Buildable sources, exact commands, compiler/environment details, input and binary hashes, raw observations and independent audits accompany the results.

## Compatibility and actual consumers

| Gate | Result |
| --- | --- |
| Workspace tests and documentation checks | 246 pass; formatting and strict workspace Clippy pass |
| Full Expat public API matrix | 3,753 pass, 987 fail; all 4,740 configurations complete without signals or timeouts |
| Change from the PR56 matrix | 12 newly passing encoding configurations; zero regressions |
| CPython 3.12.13 XML consumers | All six XML modules succeed in each of four shared/static, original/fixed configurations; 803 reported tests and 31 skips per run |
| Native C callback and allocation probes | All six shared/static runs pass; 353 allocation-failure scenarios per linkage |
| Generated and named differential corpus | 12,318 semantic comparisons pass; six exact-fragment and 450 final-position differences remain |
| W3C mandatory acceptance | 5,916 pass, six fail; 81 optional observations, no inconclusive/resolver errors |

The [full API and W3C reports](../external-grammar/) identify this same runtime. W3C failures are two descriptors (`hst-lhs-007`, `rmt-e2e-38`) at all three chunk sizes; reference Expat also accepts their catalog-required rejection cases. They remain failures. W3C validation checks nonvalidating acceptance, not canonical output.

CPython uses the actual pinned 3.12.13 consumer source. Original and fixed configurations distinguish the explicit upstream external-parser cleanup backport. Expected failures and skips remain in the logs; reported test counts are not counts of non-skipped passes. Native C callbacks use ASan/UBSan against uninstrumented Rust release libraries. Both linkage modes preserve all selected allocations through injected failures.

The generated corpus uses seed 20260910 and chunk sizes 1, 2, 3, 7, 64 and 4096. Acceptance, error codes and normalized callbacks match; the strict fragment/position gate remains failed. Independent auditing recomputes counts and verifies consumer source/copy hashes, native binary/log hashes and the full matrix deltas.

## Sustained sanitizer campaigns

The isolated snapshot contains the exact final runtime and marked grammar fuzz harness. All three Rust binaries are instrumented with AddressSanitizer. Each campaign uses one CPU, a 600-second requested duration, a 10-second per-input timeout and a 1,536 MiB RSS limit. Actual libFuzzer duration is 601 seconds per target.

| Target | Corpus input files replayed | Sustained executions | Findings |
| --- | ---: | ---: | ---: |
| `value_family` | 1,544 | 949,788 | 0 |
| `multibyte` | 7,670 | 1,976,003 | 0 |
| `ffi_family` | 6,499 | 986,686 | 0 |
| Total | 15,713 | 3,912,477 | 0 |

The corpus count includes two empty files; libFuzzer's three empty-input bootstraps produce 15,714 replay executions. Another 84 stress replays cover large values/grammar/metadata, encodings, direct and buffered feeds, child lifetimes and selected-allocation failures. Every replay and campaign exits successfully without artifacts. Source, source-manifest and executable hashes match before and after each campaign.

Independent review verifies all 299 frozen source entries, build commands and ASan symbols, exact input archives, output logs, run statistics and final artifact hashes. LeakSanitizer is disabled under the host's tracing environment; selected-allocation liveness assertions and release accounting remain active. These bounded campaigns supplement source review and deterministic regressions; they do not establish exhaustive safety.

## Allocation failure classification

A separate diagnostic overlay raises only 69 retry-ceiling constants in the adapted allocation suites to 512, preserving the assertions. Across 696 selected configurations, it reports 620 diagnostic passes and 76 failures: 554 formerly failing configurations now pass, and no original pass becomes a failure. A separate constructor ceiling experiment clarifies another 12 failures. These experiments do not modify the original suite or its **987 failures**.

The nine test names still failing after the first overlay consist of 12 constructor retry-ceiling cases, 12 nested-child failure-stage cases and 52 cases expecting Expat's particular realloc or initialization schedule. With an allocation budget of 12, reference Expat creates a child and fails during its parse; Oriole exhausts that budget at child creation. The exact nested DTD succeeds in both libraries with no fault injection, across all 12 configurations, and releases all selected allocations. Full source excerpts, overlays, diagnostic results and the 48-observation stage probe are retained.

Other API differences include unsupported configuration controls, resource ceilings, malformed-input error codes, exact callback boundaries and positions, input-context availability and encoding contracts. These require explicit consumer-specific acceptance; allocation diagnosis does not waive them.

## Performance and deployment scope

[Fresh benchmarks](../../../../benchmarks/results/2026-09-10/final-runtime/) measure this source through the native C and safe Rust APIs, including system/jemalloc/mimalloc allocation and DTD scaling. Oriole remains about 5–11 times slower than Expat on the 4 KiB generated C workloads.

The [complete final PBS build](../pbs-final/) passes every distribution gate using this runtime: the archive validator, custom checks, installed XML suites and the actual glibc 2.17 baseline. The installed suites report 802 tests with 12 skips; glibc 2.17 reports 802 tests with 13 skips and completes 1,024 threaded parses. Downloaded artifact digests and the archive's embedded bundle manifest match their recorded identities. This remains a separate gate from the four local consumer builds.

The project remains experimental. Independent review of all 32 non-allocation failing test names (357 configurations) found no new observed successful-input data loss, ownership flaw or acceptance/security bypass. The evidence supports a controlled opt-in CPython 3.12.13 Linux x86-64 trial within the documented contract. This is a bounded assessment, not a safety certification. Complete Expat API compatibility, exact callback/position equivalence, some custom-encoding contracts and distribution validation on additional targets remain open. The documented C interface performance gap also remains. The [C interface contract](../../../../crates/oriole_expat/) and [production acceptance criteria](../../../../CONTRIBUTING.md) define these limits.

## Retained evidence

- `source.tar.gz` and `source-index.json`: buildable parser, header, benchmark and consumer harness source from the recorded Git snapshot.
- `consumer-and-native.tar.gz`: four actual CPython configurations, six native runs, source/build manifests, logs and independent audit.
- `generated.tar.gz`: full corpus, both engines' observations and summary, including strict differences.
- `campaigns.tar.gz`: frozen source, exact initial/evolved corpora, 84 stress inputs, build provenance, compressed logs and independent reviews.
- `allocation-diagnosis.tar.gz`: assertion-preserving overlays, full diagnostic results, source classifications and child-failure-stage probe.

Archive member hashes and `SHA256SUMS` make the retained packages independently checkable. Executable hashes and reproducible build commands are retained; binaries remain in the recorded build locations rather than in Git.

# Namespace callback-name experiments

These draft PRs test two ideas from Expat's namespace implementation: retain URI bytes in callback storage and reuse validation already performed by the scanner. A third draft tests identifying unchanged default bindings without comparing URI bytes. A fourth isolates the scanner proof directly on the qualified runtime.

| Draft | Source | Change |
| --- | --- | --- |
| [#173](https://github.com/astral-sh/oriole/pull/173) | `c3780fa` | Bounded callback-name buffer retaining the previous URI prefix |
| [#174](https://github.com/astral-sh/oriole/pull/174) | `8b849fec` | Reuse the completed scanner's Name proof; check the remaining QName rules |
| [#175](https://github.com/astral-sh/oriole/pull/175) | `5cd5c6ec` | Checked default-binding revisions, with byte comparison on overflow |
| [#176](https://github.com/astral-sh/oriole/pull/176) | `73594809` | Scanner proof only, based directly on #172 |

PRs #173–#175 extend the existing native stack. PR #176 is an alternative based directly on #172; it forms a separate stack with this evidence report. The selected controlled-trial runtime remains `5d983f7e`. None of these measurements changes the project README's qualified benchmark or readiness claims.

## Performance

Each row covers the 24 real-project conditions, using the complete source at that draft against the qualified runtime and Expat 2.8.4. The four generated conditions are excluded from this table and the 20% aggregate criterion. Lower time ratios are better. The sources were measured in separate epochs; differences between rows do not establish each PR's marginal benefit.

| Source | Native / qualified | Native / Expat | Native conditions faster | CPython / qualified | CPython / Expat | CPython conditions faster |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Storage (#173) | 1.0064 | 1.1895 | 11/24 | Not run | Not run | — |
| Storage + Name proof (#174) | 0.9883 | 1.1689 | 17/24 | 0.9929 | 1.0479 | 15/24 |
| Storage + Name proof + revisions (#175) | 1.0004 | 1.1803 | 9/24 | 0.9950 | 1.0494 | 9/24 |
| Name proof only (#176) | 0.9915 | 1.1720 | 15/24 | 0.9943 | 1.0500 | 14/24 |

The storage-only change does not demonstrate a gain. The cache-plus-scanner-proof composition records 1.17% less native time and 0.71% less CPython time. Its native Maven namespace cases improve 3.9–5.3%, and DocBook namespace cases improve 2.3–4.5%; GTK namespace cases regress 1.8–1.9%. Vulkan ElementTree at 64 KiB regresses 2.35%. The revision composition improves CPython aggregate time by 0.50%, with 15/24 conditions slower. It is effectively flat natively, with GTK namespace regressions of 2.5–3.4%. It has not established a benefit from the added state.

The lean QName-only draft is the recommended next review candidate. It records 0.85% less native time and 0.57% less CPython time, close to the larger composition, with only +16/−6 runtime lines and no new cache state or allocations. Its native Maven namespace cases improve 4.0–4.7%, and DocBook improves 2.9–3.8%. All nine native and ten Python adverse conditions remain visible; the largest Python regression is Wayland pyexpat at 4 KiB, 2.25%. It needs confirmation before any change to the selected trial runtime.

These are small changes on a shared host, with one seven-round epoch per source and mode. No significance or repeatability claim is made. All condition rows, including adverse results and the four generated conditions, remain in the copied evidence. The 20% target applies to the real-project aggregate; individual projects remain farther from Expat. The generated-condition aggregates remain 3.35–3.51× Expat across these experiments (3.3527× for the lean draft).

### Measurement method

Six pinned project XML files, two feed sizes (4 KiB and 64 KiB), and two namespace modes produce 24 native conditions. CPython uses the same files and feed sizes through unmodified CPython 3.12.13 ElementTree and pyexpat consumers. These measure parsing those XML files, not whole applications.

Oriole uses generic x86-64 O3, ThinLTO and one codegen unit through the Ohm toolchain with its experimental defaults disabled. Expat uses the existing normal GCC O3 build. No PGO is used. Native and Python libraries, compiler arguments, source hashes, canonical callback/output checks, and Python module/parser origins were checked independently. Adjacent text callbacks are coalesced for performance preflights; strict callback assertions are tested separately.

Workers run sequentially on CPU0, with builds and sanitizer runs outside the elapsed measurements. Each condition uses the median of seven paired process-median ratios; aggregate ratios are equally weighted geometric means. Native runs contain 672 workers and 106,176 samples each; each Python run contains 576 workers and 26,364 samples. Inputs, imports and canonical checks are outside timed regions; parser construction, feeds, finalization, callbacks and explicit result destruction are included.

## Correctness and adversarial checks

The three-draft composition `5cd5c6ec` preserves all 4,740 upstream API outcomes: **4,349 passes, 391 failures, zero timeouts**, with no ordered-row or assertion-line changes. Strict shared/static CPython, using the existing external-parser cleanup patch (`--consumer-fix`), each preserve 802 methods, 809 rendered outcomes, and the same two grouping failures (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`). Raw exits 1/2/2 remain recorded; this is unchanged compatibility, not a clean upstream-suite pass.

Its three Rust-instrumented ASan harnesses (`streaming`, `ffi`, `multibyte`) pass **45,181 replay executions and 366,981 mutation executions**, with zero sanitizer/panic/timeout signatures or crash artifacts. Each exploration runs for 60 seconds. Compiler evidence verifies instrumentation in the parser, selected-allocation storage and C interface, plus the harness dependencies. All 12 commands and controller processes completed and were reaped.

The lean `73594809` source separately preserves the same complete API and strict CPython outcomes, with every ordered row, assertion and rendered outcome unchanged.

The lean source also passes **45,181 replay executions and 367,176 mutation executions** across the same three Rust ASan harnesses, with zero sanitizer findings or crash artifacts. Its fresh instrumented build and all 12 campaign commands complete successfully; source and artifact checks are retained in its separate readback.

The harnesses cover owned incremental parsing, C API state/allocator safety, and custom multibyte decoding. Deterministic tests and compatibility suites check adapter semantics. Leak detection is disabled; these bounded campaigns are not exhaustive safety proofs. Neither source has a new W3C or installed PBS run.

The core remains safe Rust. In the cache experiments, callback names are owned by detached frames; no borrow into namespace bindings escapes parsing. The name buffer is capped at 4 KiB and shares the existing retained-capacity budget. URI expansion work is charged on every tag, including cache hits. Revision overflow permanently falls back to byte comparison, and parser-generation checks exclude frames from other parsers or reset instances.

The storage and cache-plus-proof normal builds pass 484 and 485 Rust tests; the revision composition passes 487, and the lean alternative passes 484. Across the drafts, tests include URI changes and scope restoration, Unicode/QName rules, separator/triplet modes, incremental input, selected allocation failures, callback pointer lifetimes, foreign frames and forced revision overflow. Formatting and Clippy pass for each source. The original storage preparation's mutable-fixture compile error and incorrect triplet-setter test assumption remain recorded as failed attempts; both were corrected before its passing build.

## Evidence and scope

The [copy index](COPY_INDEX.json) identifies exact copies of build records, compiler/test logs, paired benchmark summaries, per-condition CSV files, source/build identity checks, and qualification reports. The [raw-evidence guide](LOCAL_RAW.md) records development-machine locations for full workers, binaries and corpora, which are not included here. Controllers preserve their original absolute paths; this is an audit package rather than a relocatable benchmark runner.

All 15 ordinary CI checks pass at each exact source commit: [#173](ci/pr173.json), [#174](ci/pr174.json), [#175](ci/pr175.json), and [#176](ci/pr176.json). Both callback-aliasing Miri modes pass for all four drafts; the PGO job is skipped. This evidence PR contains documentation and preserved reports only, and its own workflow status is separate.

These drafts support review of these tradeoffs. They do not certify exhaustive memory safety, full Expat interchangeability, or a new installed PBS distribution. The prior qualified runtime and its documented compatibility differences remain the deployment baseline.

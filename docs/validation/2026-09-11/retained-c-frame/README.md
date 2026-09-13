# Retained C adapter frame — rejected

Retaining the callback arena across successful unfinished feeds did not provide a broad native PGO improvement. Keep the suffix-sharing element-name runtime selected: the candidate adds 256 bytes to every parser handle, while real-project elapsed time is effectively flat and all four generated conditions regress.

Candidate: `c44f3b3e7dbb9dc25e4eb361e0bfef1735c934d7`. Measured control runtime: `a0acaf1bf8c8ae3f55030332be794cb8b9b5bea9` (the suffix-sharing implementation). Only `crates/oriole_expat/src/lib.rs` and its `tests.rs` differ in the 72-file build source set. This PR publishes evidence only; the prototype remains unselected.

## Native PGO results

| Scope | Candidate / selected control | Candidate / Expat | Adverse conditions |
| --- | ---: | ---: | ---: |
| 24 real-project conditions | 0.99949× (effectively flat) | 1.35311× | 14 / 24 |
| 4 generated conditions | 1.00686× (+0.69%) | 5.04598× | 4 / 4 |

The unchanged campaign contains seven paired runs for six real XML projects, 4 KiB and 64 KiB chunks, with namespaces on and off. Four generated conditions remain separately reported. The independent reader reconstructed 672 worker records and 106,176 samples. The largest real regression is Vulkan, 4 KiB, namespaces off: +3.73%. Every condition, including regressions, is retained in [conditions.csv](conditions.csv). These are parser/consumer benchmarks, not whole-application timings. Ratios are calculated from the established paired median procedure; the aggregate is a geometric mean across conditions.

Both libraries use the existing O3, ThinLTO, one-codegen-unit configuration and fresh PGO trained only on the original generated training corpus. The selected allocator and benchmark protocol are unchanged. A normal optimized build and its generated replay also completed, but **normal elapsed benchmarks, CPython elapsed benchmarks, and strict CPython tests were not run for this candidate**. The rejected PGO result did not justify expanding those campaigns.

## Allocation and handle costs

The checked x86_64 layouts measure `XML_ParserStruct` at 3160 bytes before and 3416 after (+256 bytes, +8.10%); alignment remains 8. The inline `Option<AdapterFrame>` is 256 bytes and creates no separate allocation. The type-size check linked the existing checked debug libraries and evaluated only `size_of`/`align_of`; it predates composition onto suffix, whose core `Parser` has the same 2408-byte size.

The unchanged 4 KiB allocation probe compared 12 fixtures against the selected suffix library. Its independent review reconstructed 24 traces, 148 feed records, allocation histograms, callback hashes and every between-feed memory delta. Maven's allocation requests decreased from 307 to 263 without namespaces and from 584 to 557 with namespaces. This reduction did not translate to a broad elapsed improvement.

Every final retained total increases by 256 bytes. Maven's between-feed increases reach 778 bytes without namespaces and 699 bytes with namespaces; its peak increases are 685 and 606 bytes. Other fixtures increase by 256 or 311 bytes between feeds and by 256 bytes at peak. Finalization releases the arena before the final retained summary, so the individual `CALL` records are necessary to observe its storage. All callback outcomes match and all tracked allocations are freed. See [allocations.csv](allocations.csv) and the raw records in the archive.

These are tracked allocation bytes, not RSS. Maven phase 2 covers its entire parse; generated phase 2 follows an explicit warmup split. Some generated fixtures therefore have two feeds even when they fit in one ordinary chunk. The two long-name inputs straddle an input-buffer boundary; only matching fixtures should be compared. No 64 KiB allocation diagnostic was run.

## Compatibility and ownership

The composed candidate passed 441 local Rust checks, Clippy and formatting. Its original upstream API campaign retains all 4740 configurations and all original assertions and limits. Ordered raw BEGIN/RESULT records exactly match the suffix control: 4347 pass, 391 assertion failures and two `test_misc_input_2gb` SIGALRM timeouts. The API suite exits 1; it is not green.

All six C consumers passed: integration, adversarial and allocation-failure consumers, each dynamically and statically linked. The allocation sweep contains 327 scenarios per linkage. C consumers use ASan and UBSan; the Rust release library is uninstrumented, and leak checking is disabled. Their library bindings are supported by compile commands, ELF metadata and hashes; these consumers do not print runtime `dladdr` origins. The upstream API run does record its loaded library origin.

The bounded lifecycle review found no new ownership defect. The frame is detached before event callbacks, cached only after successful unfinished parsing with an inactive payload, and finished on completion, suspension and event-loop errors. Successful reset finishes it against the old core; failed reset preserves it transactionally. Early feed failures and idle stop may retain the charged inactive cache until resume/reset/free. Its reservation remains inside the existing 64 KiB aggregate cache cap. This ownership evidence does not justify accepting a larger handle without a useful performance gain.

## Saved evidence

`report.json` records the decision and exact audit paths/hashes. `evidence.tar.gz` contains raw records, controllers, source snapshots and independent readbacks; `index.json` lists each archive member, its original path, size and SHA-256. No executables, shared/static libraries or binary profiling files are included. Their recorded identities remain in the build and run reports. Absolute paths inside original receipts describe the original machine; this packet is a saved-record bundle, not a new executable verification framework or a self-contained benchmark installation.

Normal/Python controller preparation files may exist in the original study directories. Preparation is not execution: this report claims only the completed campaigns listed above. The selected suffix runtime's earlier CPython results do not apply to this rejected candidate.

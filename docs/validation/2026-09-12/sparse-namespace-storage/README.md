# Sparse namespace storage

Retain the smaller element record with modest, consistently favorable aggregate observations across two normal-build campaigns. The measured 74-file source matches runtime `b9190cda8b04092a5691d8ec1cbefbc116f9e4e4`, based on published Text `19b28544`. Confirmation observes **1.35% less native real-input time and 1.11% less CPython time** against that Text control. Namespace-enabled native time is essentially flat. Native and Python remain **1.4461×** and **1.1860× Expat**, above the roughly 1.10× goal; five of 24 Python conditions meet it. These shared-host observations do not establish statistical significance or production readiness.

## Implementation and memory tradeoff

A parser-owned sparse stack stores only nonempty namespace binding blocks; each Element holds an optional scalar index. Start keeps local binding construction and namespace-map/event ordering, reserves both owners before committing, and retains existing name moves. End detaches its scope block before fallible event emission or reverse restoration so that failures drop the same owned bindings. Scope-free child elements leave ancestor blocks untouched; external children begin with a fresh pool.

The pool releases its allocation when empty if capacity exceeds a 4 KiB metadata threshold. An active ancestor can retain peak pool storage, and namespace declarations at every depth can add metadata. The change adds no unsafe code, public API, dependencies or SIMD and retains name ownership and source compaction.

Only `crates/oriole/src/lib.rs`, `crates/oriole/tests/adapter_frame.rs` and `crates/oriole/tests/allocation.rs` differ from the base. [committed-source.json](committed-source.json) binds all 74 measured source hashes to the later commit without claiming a rebuild after commit. Source reviews cover joint reserve/commit ordering, detached End ownership, scope-free descendants, external children, retention and allocation-failure cleanup.

Actual debug Element size falls **168→120 bytes**; Parser grows **2408→2464 bytes (+56)**. Separate optimized disassembly shows the End-frame transfer changing from a 168-byte memcpy to 120 bytes of inline loads/stores. Both versions still copy the 72-byte ElementName afterward. These artifacts establish layout and movement differences, not the cause of the elapsed result. Private heap/RSS was not measured.

## Separate normal campaigns

| Epoch / campaign | Candidate / control | Candidate / Expat | Lower-time conditions |
| --- | ---: | ---: | ---: |
| confirmation: Native real XML | 0.986549× | 1.446066× | 16 / 24 |
| confirmation: Native namespaces off | 0.973051× | 1.384195× | 11 / 12 |
| confirmation: Native namespaces on | 1.000234× | 1.510703× | 5 / 12 |
| confirmation: Native generated | 1.000792× | 3.887751× | 2 / 4 |
| confirmation: CPython all | 0.988884× | 1.186026× | 19 / 24 |
| confirmation: ElementTree | 0.988914× | 1.246176× | 9 / 12 |
| confirmation: pyexpat events | 0.988854× | 1.128779× | 10 / 12 |
| initial-shared-host: Native real XML | 0.975283× | 1.457056× | 20 / 24 |
| initial-shared-host: Native namespaces off | 0.969519× | 1.398269× | 10 / 12 |
| initial-shared-host: Native namespaces on | 0.981082× | 1.518314× | 10 / 12 |
| initial-shared-host: Native generated | 1.008702× | 4.009998× | 1 / 4 |
| initial-shared-host: CPython all | 0.990616× | 1.188056× | 22 / 24 |
| initial-shared-host: ElementTree | 0.988493× | 1.249534× | 11 / 12 |
| initial-shared-host: pyexpat events | 0.992743× | 1.129603× | 11 / 12 |


Every ratio is the equally weighted geometric mean of condition-wise medians of seven paired process ratios. The two epochs are not pooled, and their gains are not added. The original and confirmation campaigns use identical libraries, inputs, conditions, seeds, callbacks and measurement boundaries. Confirmation builds no modules: all six previously verified O2 extensions are reused in place, with fresh preflights and timed workers.

Confirmation retains eight native real-input regressions (largest **2.02%**, Maven 4 KiB with namespaces), two generated-entity regressions (largest **3.38%**) and five Python regressions (largest **0.57%**, DocBook 64 KiB ElementTree). The initial campaign retains four real Batik regressions, three generated regressions and two Python regressions. All **104 condition rows / 24 adverse rows** remain in the full and adverse CSVs, with exact values in [report.json](report.json).

The featured README sample comes only from confirmation, 4 KiB feeds with namespaces off:

| Project XML | Oriole (normal) | Expat (normal) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 46.112 ms | 28.947 ms | 1.593× |
| Wayland protocol | 1.092 ms | 1.073 ms | 1.018× |
| Maven POM | 0.768 ms | 0.437 ms | 1.768× |
| Batik SVG | 0.132 ms | 0.134 ms | 0.984× |
| GTK UI | 0.323 ms | 0.214 ms | 1.510× |
| DocBook XSL | 0.259 ms | 0.189 ms | 1.372× |

Times are medians of seven process medians; ratios are medians of seven paired process ratios and need not equal division of displayed times. Both native aggregates still exceed the goal, and the generated controls remain approximately 3.89× Expat in confirmation.

Each epoch has 28 native conditions, seven pairs, 84 preflights and 588 timed workers: **672 workers / 106,176 samples**. Python has 24 conditions, 72 preflights and 504 timed workers: **576 workers / 26,364 samples**. Across both epochs, totals are **2,496 workers / 265,080 samples**. Counts include saved preflight samples and warmups; only timed iterations enter elapsed medians. Native timers include the complete parser lifecycle. Python timers include construction/feed/finalization/callbacks and explicit destruction; imports, input reads, canonical checks and explicit collection stay outside, with automatic GC enabled. Original timeouts and process-group cleanup remain unchanged.

Oriole uses normal generic O3, ThinLTO and one codegen unit with Ohm rustc 1.98.1-dev and automatic Ohm defaults disabled. The pinned Expat 2.8.4 control uses GCC 13.3 O3 **without LTO**. Unmodified CPython 3.12.13 extensions use O2. Original build vectors, physical compiler identities, source and library hashes are retained. No new PGO or CPU-target experiment was performed.

## Host observations

The initial campaign ran while another Toucan arena task used unrestricted Cargo builds/tests on the same `charlie-oss-6` host. CPU0 affinity and sequential Oriole targets did not establish an idle host. Its complete results remain immutable and separate.

Before confirmation, the other task reported its performance comparison finished. Ten-second observations before native release showed 99.40% CPU0 and 99.65% aggregate idle+iowait; before Python release these were 97.69% and 96.99%. Five-second monitoring during fully contained elapsed intervals observed other CPUs averaging **94.22% idle+iowait natively** and **99.12% during Python**. Raw counters, release records and independent attribution are archived. This was a monitored shared-host window, not OS-enforced isolation or a fully idle host. Samples do not cover every instant, identify every process, or rule out cache, frequency and memory-bandwidth interference; no statistical-significance claim is made.

## Compatibility and remaining limits

All **451 workspace tests across 35 groups**, formatting and strict Clippy pass. Original **4,740 API configurations** remain exact: **4,347 pass / 391 assertion failures / two timeouts**, raw exit 1, with unchanged assertions and bounds. Six shared/static C integration, adversarial and allocation consumers pass. Their C harnesses use ASan/UBSan; Rust normal libraries remain uninstrumented and leak detection is disabled.

Each strict CPython linkage retains **802 method outcomes and 809 rendered outcome lines**, raw exit 2, including `test.test_pyexpat.BufferTextTest.test1` and `test.test_sax.CDATAHandlerTest.test_handlers`. Strict consumers carry the explicit upstream allocation-failure cleanup backport; benchmark consumers are unmodified. Two supplemental semantic tests pass per linkage and do not erase those strict failures. Benchmark callback coalescing checks a narrower contract than exact callback grouping. Historical relaxed-ceiling diagnostics, sustained sanitizer campaigns and PBS/glibc trials keep their original source identities. No current namespace sustained fuzzing or installed PBS validation is claimed.

## Preparation and review history

Root's source review caught a reserve-error conversion issue before compilation: both new `try_reserve` calls needed the existing `AllocError::from` mapping. Revision 1 and corrected revision 2 are preserved; this was not a failed compiler target. Root formatting changed no source bytes. ENOSPC prevented one independent source-review command from launching. Four clean obsolete worktrees were removed with commits, branches, raw evidence and shared cache retained. The disk-recovery receipt does not establish concurrency or later host quietness.

The independent raw reviewer retained a tuple-versus-JSON-list comparison correction and its first failure before passing the complete audit; this changed the reader only. The initial report's compiler wording was corrected to distinguish Oriole ThinLTO/codegen units from Expat's GCC O3 recipe. Historical pending drafts retain their labels alongside completed evidence. No target attempt was removed to hide a regression.

The namespace implementation author prepared this packet and adapted build/native/correctness helpers. Root ran targets and primary saved readers. Separate agents reviewed source/protocols, independently reconstructed the initial and confirmation raw results, bound the commit and inspected actual layout/host records. Final archive/documentation review is separate. All historical packets remain unchanged.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 5,494 indexed files (13,197,935 compressed bytes). [index.json](index.json) records every member hash, original path, size and excluded executable/library identity. Every archived member and original was read back after writing. [conditions.csv](conditions.csv) includes all 104 rows from the two separate namespace epochs; [adverse-conditions.csv](adverse-conditions.csv) includes all 24 adverse conditions (17 native; 7 Python). [report.json](report.json) retains full precision and limits.

Archive SHA-256: `161a2ab2598a9387ca80cd1b2d88214f14134380601904b6500578cbfade7667`. Executables, libraries, compiler profiles and machine configuration are excluded. The source member map reconstructs the namespace candidate and selected Text control snapshots. The isolated Text experiment is outside this packet.

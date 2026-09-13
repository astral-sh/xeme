# Fused attribute scan — rejected

**Reject this candidate.** Its PGO native gain of 1.16% does not justify the observed CPython regression: 1.39% overall and 2.73% for ElementTree, with 11 of 12 ElementTree conditions slower. Pyexpat is effectively flat. The selected parser stays unchanged.

Only `crates/oriole/src/tag.rs` differs from the selected source. The uncommitted candidate is based on publication `4064b0653534aff690c0e0f395a6b3b48da878fc`; its exact final file SHA-256 is `50f641a4eb62b6deb538851f60ea66ac425eceeb9cce539a27f5bc0ffc9a8de6`. The selected measured source is `0f66d54ac8418f0a6e628ad18570677a19c9ed45`. Source manifests and full snapshots bind the actual bytes; the base commit alone does not identify the candidate.

## Results

Ratios compare elapsed time; below 1 is faster. “Adverse” means the candidate takes longer than the selected control.

| Scope | Candidate / selected control | Candidate / Expat | Adverse conditions |
| --- | ---: | ---: | ---: |
| Native normal, 24 real | 0.975392× (-2.46%) | 1.553879× | 1 / 24 |
| Native PGO, 24 real | 0.988360× (-1.16%) | 1.311669× | 7 / 24 |
| Native normal, 4 generated | 0.988464× (-1.15%) | 4.069895× | 1 / 4 |
| Native PGO, 4 generated | 1.003935× (+0.39%) | 3.884945× | 2 / 4 |
| CPython PGO, all | 1.013909× (+1.39%) | 1.133378× | 17 / 24 |
| CPython PGO, ElementTree | 1.027322× (+2.73%) | 1.198143× | 11 / 12 |
| CPython PGO, pyexpat | 1.000671× (+0.07%) | 1.072114× | 6 / 12 |

[native-conditions.csv](native-conditions.csv) retains all 56 native conditions, including the four generated controls per build. [python-pgo-conditions.csv](python-pgo-conditions.csv) retains all 24 Python conditions. [report.json](report.json) records the reviewed aggregates, identities and limits. Native PGO remains about 31% slower than Expat in this campaign; the within-10% goal is not met.

Native uses six held-out real projects, namespaces off/on, 4 KiB and 64 KiB feeds, and four original generated conditions. Python uses the same six projects and feed sizes with ElementTree and pyexpat event consumers. Each condition retains seven seeded, interleaved process pairs. Condition ratios are medians of paired process-median ratios; aggregates are geometric means across conditions. Independent saved-data readers reconstructed all 1,344 native workers/212,352 observations and 576 Python workers/26,364 observations, including preflights and warmups. No rows or adverse conditions were dropped.

Native timers include parser creation, feed/finalization, callbacks and destruction; input loading and dynamic-library loading are outside. Python timers include parser creation, feeds, finalization, callbacks and explicit destruction; imports, input reading, canonical checks and explicit collection are outside. Automatic GC remains enabled. Python extensions use unchanged CPython 3.12.13 source without the consumer-cleanup backport, compiled with GCC 13.3 at O2. The campaigns ran on Linux x86_64 with an AMD EPYC-Milan processor. These are parser/consumer measurements, without whole-application or statistical-significance claims. The data does not establish the cause of the ElementTree regression.

## Source and build

The change fuses native attribute quote/delimiter scanning with an ordinary-ASCII value proof. Proven values skip a later eligibility scan. General grammar, non-ASCII support and incremental fallback timing remain intact; the change adds no allocation, cache, allocator choice or CPU-specific flags. Focused scanner/planner regressions require completed valid-Unicode plans and preserve the incomplete-to-fallback boundary for an ampersand.

Both Oriole builds retain O3, ThinLTO, one codegen unit, the selected allocator and generic `x86_64-unknown-linux-gnu`. Local builds use `cargo +ohm -Zohm-defaults=no`: the exact receipt identifies `rustc 1.98.1-dev (f62703110 2026-09-08) (ohm-1.98.1-1)`, LLVM 22.1.8. Fresh PGO uses the original generated training recipe; all six real projects remain held out. Nine actual compiler vectors and 864 generated parse records were reviewed against the selected recipe. Expat 2.8.4 uses the retained matching normal or PGO GCC O3/shared controls, without LTO.

Candidate shared libraries are normal `0ce2320a098436307472ba809b0053a910573f830db01d7bc7c5c6ca99ec958c` and PGO `202ae85b10301a52276b4ac7be9134b9eeb2d7d728706f380149e4734304b699`; the PGO static archive is `e69363d0cf2a9439bed0b86c9a200ff26f2752a6b39261b031d24eebe69bd163`. Selected normal/PGO shared controls are `087d131c215012d2a93527a4ec66a716216e8ab4009948a1f79d0a7644d5d949` / `d3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4`. Full compiler, profile, Expat and extension identities remain in the saved receipts.

## Correctness and limits

Final local checks passed 442 Rust checks in 35 groups, strict workspace/all-target Clippy and formatting, with zero failed or ignored tests. The first formatting and test-style Clippy failures are retained, together with the corrected final-source run. Debug DWARF reports AttributeScanner 48 bytes, TagScanner 96 and Parser 2408, matching the selected debug binary; this is object-layout evidence only, without a release-layout, heap or RSS claim.

The original upstream matrix remains byte-identical to the selected result: **4,347 pass, 391 assertion failures and two timeouts across 4,740 configurations**. Its nonzero suite exit, original assertions, retry ceilings and 3-second/1-GiB/768-MiB per-context limits are preserved. A successful comparison means no outcome changed; it does not turn those failures into passes. Six C integration/adversarial/allocation consumers passed, with shared and static linking. ASan/UBSan instrumented the C consumers only; the Rust PGO library was uninstrumented and leak checking was disabled.

The candidate was rejected after the PGO Python results. **Normal Python, strict shared/static CPython and supplemental text-fragmentation checks were prepared but never executed on this candidate.** No new candidate CI or sustained sanitizer/fuzz campaign is claimed. Successful performance preflights compare canonical text payload/order and do not establish exact callback fragmentation compatibility. Original selected strict failures remain separate.

## Saved evidence

[evidence.tar.gz](evidence.tar.gz) contains final source and patch, earlier source/check attempts, build/training records, all completed native/Python raw records, original API and six C-consumer outcomes, readers and reviews, input bytes, licenses and notices. Unexecuted controllers are stored under `prepared-unexecuted`; combined preparation files retain their historical stage labels. [index.json](index.json) records each member's original path, size and SHA-256; packaging read back every member.

The source author prepared the candidate and build/correctness adaptations. Root executed all targets and reviewed the build binding. A separate agent reviewed runtime/build source and prepared the native/Python saved-data readers; root executed those readers. That agent directly ran the saved API/C readback. This packet assembly is not a second target execution. Two packaging attempts stopped before archive creation on study-relative Python pins and a shared-library soname symlink. Their scripts/errors and the corrections are retained. The first completed packet is also retained locally with its original hashes; this revision clarifies environment and reviewer roles only. Executables, libraries (including their soname links), binary profiles and machine configuration are excluded with receipt identities retained. Absolute paths identify the original machine; this is a saved-record bundle rather than a self-contained runnable benchmark.

A source-only note about a possible eager position proof is retained as an **unimplemented hypothesis**. Older Context PGO instruction records put a broader markup-position region at 2.64–3.68% in four non-namespace cases; that includes ineligible/other markup and is neither removable cost nor a current elapsed estimate. It does not justify selecting another runtime or claiming the remaining performance gap can be closed.

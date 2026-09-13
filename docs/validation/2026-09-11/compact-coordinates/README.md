# Compact lazy coordinates — rejected

**Reject this candidate.** Real-project native elapsed regresses by 3.33% in the normal build (23 of 24 conditions) and 7.84% with PGO (all 24). Generated aggregates regress by 6.41% and 12.05%. Keep direct-reference frames selected.

Measured candidate: `e1228bbb9dbd174e2aceff5ecea24a04fecc8515`. Selected control: `0f66d54ac8418f0a6e628ad18570677a19c9ed45`, published in [PR144](https://github.com/astral-sh/oriole/pull/144) at `4064b0653534aff690c0e0f395a6b3b48da878fc`. This packet publishes evidence only; the compact-coordinate runtime remains unselected.

## Native results

| Build and scope | Candidate / selected control | Candidate / Expat | Adverse conditions |
| --- | ---: | ---: | ---: |
| Normal, 24 real conditions | 1.033262× (+3.33%) | 1.650196× | 23 / 24 |
| PGO, 24 real conditions | 1.078406× (+7.84%) | 1.432536× | 24 / 24 |
| Normal, 4 generated conditions | 1.064074× (+6.41%) | 4.356341× | 2 / 4 |
| PGO, 4 generated conditions | 1.120530× (+12.05%) | 4.338143× | 4 / 4 |

Generated entity/reference conditions regress 14.56–15.54% in normal builds and 24.20–24.92% with PGO. PGO Maven regresses 12.18–13.43% and DocBook 8.37–13.99%. All 56 condition ratios, including every adverse condition, are retained in [conditions.csv](conditions.csv); [report.json](report.json) includes the paired-condition arithmetic and per-project aggregates.

Each mode uses the same six real-project inputs, 4 KiB and 64 KiB chunks, namespaces off/on and four generated conditions. Each condition retains seven seeded, interleaved process pairs. A condition ratio is the median of paired process-median ratios; aggregates are geometric means across conditions. Saved audits reconstruct all 1,344 workers and 212,352 observations, including preflights, callback observations, library paths and byte hashes. This is one native parser/consumer campaign per mode on this machine, without a statistical-significance or whole-application claim.

Both Oriole builds use O3, ThinLTO and one codegen unit on the same explicit x86_64 target, with the selected allocator. PGO uses fresh profiles from the original generated training recipe. Expat 2.8.4 uses matching normal or PGO GCC 13.3 O3/shared builds without LTO. Driver, fixtures, callbacks, seeded order, limits and timing arithmetic remain unchanged. This candidate moves further from the within-10%-of-Expat goal.

## Source and correctness checks

The candidate defers eligible native UTF-8 line/column projection until a coordinate getter or a boundary needs it. Byte/count publication stays eager; the existing publication owner retains committed coordinates through compaction and failures. An additional Source checkpoint replaces the prior design's larger retained-coordinate slots. This remains a rejected implementation experiment, with no claim that object size caused either design's elapsed results.

The final source manifest contains 74 files (selected control 72); the pipeline manifest contains 71 (selected 69). Six runtime files and eight test-bearing files differ, including two new focused test modules. The original freeze deliberately records dirty bytes on base `97117de`; the saved commit binding proves those exact checked files became `e1228bbb`. Final snapshots, the earlier unformatted proposal, reviewer-added test patches, and original review scope are retained separately.

Independent runtime review found no runtime blocker and identified two focused coverage gaps. The reviewer subsequently added C allocator-reentry/failed-publication and actual deferred-compaction callback tests; those test additions are not represented as independently authored runtime review. Final local checks passed 449 Rust tests in 34 groups, formatting, all-target checking and Clippy. Normal, PGO-training and PGO-use builds each completed 288 original generated parse records (864 total), with nine actual compiler vectors and six library artifacts reviewed against the selected recipe.

The first formatting check failed on the unformatted prototype; its log is preserved alongside formatting and the successful final attempt. A saved build-reader schema assertion also preceded the corrected passing review; it was a reader-shape issue, not a target failure.

## CI and object layout

The initial [CI run 34656508229](https://github.com/astral-sh/oriole/actions/runs/34656508229) could not start any jobs because a command ending in `coordinate_tests::` was not quoted as YAML. A block-scalar-only workflow correction at `be31c37baadcc711a84120c487ac13383dd0ccc7` preserves all measured parser source. The [corrected run 34656684670](https://github.com/astral-sh/oriole/actions/runs/34656684670) passed all 16 jobs. Saved full logs verify exact selection and execution of 18 focused callbacks/coordinate checks under each of Miri's Stacked Borrows and Tree Borrows models, 36 passes total.

The checked debug binary's exact layout test reports C parser 3184 bytes, core Parser 2424, AdapterFrame 264 and AdapterLocation 40. This observation measures object layout only; private heap, peak memory and RSS were not measured. Earlier design documents' Source budget and V1 layout are historical evidence, not additional current measurements.

## Scope and saved evidence

The candidate was rejected after both native campaigns. Local original upstream API/six-C-consumer, strict shared/static CPython and 12-case allocator controllers were prepared but **not executed**. Candidate Python performance and sustained sanitizer/fuzz campaigns were not run. Successful remote CI jobs use their documented test selections and CPython text-fragmentation allowance; they do not erase the selected runtime's preserved strict callback differences or establish complete API compatibility.

[evidence.tar.gz](evidence.tar.gz) retains exact source, checks and first attempts, original build/training records, CI metadata/full log, layout output, prepared controllers, all native raw worker records, saved reviews and corpus licenses/notices. [index.json](index.json) records every member's original path, size and SHA-256; packaging read back every member. Source-review caveats and prepared-state labels describe their original stage and remain unchanged.

Executables, shared/static libraries, binary PGO profiles and machine Cargo configurations are excluded; their receipt hashes are retained. Absolute paths identify the original machine. This is a saved-record bundle, not a self-contained executable benchmark. Packaging ran no parser, compiler, test or benchmark target.

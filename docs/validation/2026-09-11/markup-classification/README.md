# Ordinary markup classification — rejected

**Reject this candidate.** The normal release build is 0.51% faster across real-project native conditions, but PGO is 0.38% slower with 17 of 24 conditions regressing. Generated PGO results are effectively flat, while normal generated character references regress by 3.31% across their two conditions and all 14 paired measurements. The change does not show a consistent broad gain. Keep direct-reference frames selected.

Candidate: `b4e262db0ffd84cc73f7047b55adf568f7834651`. Measured selected control: `0f66d54ac8418f0a6e628ad18570677a19c9ed45`, published in [PR144](https://github.com/astral-sh/oriole/pull/144) at `4064b0653534aff690c0e0f395a6b3b48da878fc`. This packet publishes evidence only; the classifier remains unselected.

## Native results

| Build and scope | Candidate / selected control | Candidate / Expat | Adverse conditions |
| --- | ---: | ---: | ---: |
| Normal, 24 real-project conditions | 0.994929× (−0.51%) | 1.594791× | 6 / 24 |
| PGO, 24 real-project conditions | 1.003822× (+0.38%) | 1.328854× | 17 / 24 |
| Normal, 4 generated conditions | 1.006409× (+0.64%) | 4.123563× | 2 / 4 |
| PGO, 4 generated conditions | 0.999659× (−0.03%) | 3.885920× | 2 / 4 |

Normal gains are led by Vulkan (1.84%) and Wayland (0.72%). Five of six project aggregates regress with PGO: Batik is 1.37% slower and GTK 0.68% slower; Wayland improves 0.18%. Generated references improve 1.49% with PGO while generated declaration stress regresses 1.44%. The normal reference regressions are 3.55% at 4 KiB and 3.08% at 64 KiB, with every paired ratio above one. All 56 conditions, including every regression, are retained in [conditions.csv](conditions.csv).

Both campaigns use the same six original real-project inputs, 4 KiB and 64 KiB chunks, namespaces off/on, and four generated conditions. Each retains seven seeded, interleaved process pairs. A condition ratio is the median of paired process-median ratios; aggregates are geometric means across conditions. Audits reconstruct all 1,344 workers and 212,352 samples, including preflights, callback observations, library paths and byte hashes. These are native parser/consumer measurements on this machine, without a statistical-significance or whole-application claim.

Both Oriole builds use O3, ThinLTO and one codegen unit on the same explicit x86_64 target, with the selected allocator. PGO uses independently generated original training profiles. Expat 2.8.4 uses matching normal or PGO GCC 13.3 O3/shared builds without LTO. Driver, fixtures, callbacks, order, limits and timing arithmetic remain unchanged. The small normal improvement does not meet the within-10%-of-Expat goal, and PGO does not improve the selected runtime.

## Source, checks and builds

Only `crates/oriole/src/lib.rs` differs in both the 72-file source manifest and 69-file pipeline manifest. The candidate adds a local predicate after recognizing `<`: a present second lexical byte other than `!` or `?` selects the existing Tag scanner immediately. Lone `<`, declarations and processing instructions retain the original classification branch. No parser field, allocation, unsafe operation or new scanner is added. The 14-line source diff and exact candidate/selected snapshots are archived.

Independent source review found no actionable correctness issue. For ordinary tags, every skipped exceptional-prefix condition was already false; the same subsequent name checks, incremental scan, token bounds, decoding-error precedence, accounting and callback path remain. Existing contextual CDATA, malformed/end-tag, Unicode and custom-encoding tests cover the relevant boundary behavior. The intended behavior is identical, including errors and positions; no compatibility assertion was relaxed.

Completed checks report 440 passing Rust checks in 34 groups, formatting, all-target checking and Clippy. All 61 Rust source hashes match the candidate. Normal, instrumented and optimized builds completed with the original 288 generated parse records per phase, 864 candidate records total. Saved build review verifies nine actual compiler vectors, six library artifacts, fresh profiles, source/tool bindings, and identical generated observations. The build execution body and six pipeline tools are unchanged from the selected control.

## Scope and unexecuted campaigns

The classifier was rejected after normal and PGO native measurements. **No upstream API, CPython performance, strict CPython or allocation campaign was prepared or run for it.** There is no new full-compatibility, CPython, sanitizer, heap, peak-memory, RSS or whole-application result. The separate semantic diagnostic on the selected reference-frame runtime is unrelated and excluded.

The earlier archived proposal and suffix-build disassembly explain the attempted shortcut. Their instruction estimates concern older saved profiles and are not current candidate measurements or a causal explanation of these elapsed results. No current-source profile or code-generation mechanism is claimed.

## Saved evidence

[report.json](report.json) records the rejected decision, complete results and campaign scope. [evidence.tar.gz](evidence.tar.gz) preserves exact source snapshots/patches, checks, build and generated-training records, original native controllers, all raw worker outputs, independent reviews, and corpus licenses/notices. [index.json](index.json) records every archive member's original path, size and SHA-256; packaging read back every member once.

Executables, shared/static libraries, binary PGO profiles and machine Cargo configurations are excluded with their original receipt hashes retained. Absolute paths identify the original machine. This is a saved-record bundle, not a self-contained executable benchmark or a new verification framework. Packaging executed no parser, compiler or benchmark.

# C input-reservation experiments

All three candidates were left unselected. Their PGO builds took more time across the real-project suite. The control runtime is `de859c89577b2982d59b6923d682032f6a7c7800`; our target remains roughly 1.10× Expat time.

| Candidate | Build | Real XML / selected | Generated / selected | Real XML / Expat | Adverse real conditions |
| --- | --- | ---: | ---: | ---: | ---: |
| Reservation pressure (`60b3a0f`) | Normal | 1.0032× | 1.0065× | 1.5883× | 18/24 |
| Reservation pressure (`60b3a0f`) | PGO | 1.0153× | 1.0138× | 1.3883× | 21/24 |
| Cold helper (`f96bf11`) | Normal | 0.9973× | 1.0072× | 1.5830× | 7/24 |
| Cold helper (`f96bf11`) | PGO | 1.0301× | 1.0224× | 1.4136× | 21/24 |
| Decoder boundaries (`49b9576`) | Normal | 1.0112× | 1.0125× | 1.6046× | 22/24 |
| Decoder boundaries (`49b9576`) | PGO | 1.0308× | 1.0187× | 1.4124× | 20/24 |

Ratios above one mean slower. These are three separate campaigns, each paired against the same selected libraries and Expat controls. They are not direct paired comparisons between candidates. All 168 condition results are retained in [conditions.csv](conditions.csv), including generated controls and regressions.

## Changes

The first candidate preserves the producer's requested buffer size across `XML_ParseBuffer` calls and uses successful reservations to decide when to retry an incomplete token. It also accepts zero-length parsing with a reservation retained across reset. The reservation controls scheduling; actual allocation limits remain unchanged.

The second candidate moves the retained-input calculation into a cold helper. Emitted code confirms the intended inlining change, but its measured PGO cost increased.

The third candidate restores the selected token and event paths and checks reservation pressure at decoder boundaries, including successful custom-encoding callbacks. Its additional regression covers pressure following custom-encoding conversion. Passing functional checks does not establish a speed advantage.

[PR #138](https://github.com/astral-sh/oriole/pull/138) retains these experiments as documentation only. Its runtime and test sources match the selected `de859c8` control; the rejected implementations remain in the archived source snapshots and patches. The two refinements were rejected without CPython timing; their prepared consumer scripts were not executed. No candidate is presented as meeting the production or performance goals.

## Measurement

Each mode contains 24 real-project conditions: Vulkan, Wayland, Maven, Batik, GTK and DocBook, each with 4 KiB and 64 KiB chunks and namespaces both disabled and enabled. Four generated conditions remain separate. Each condition uses seven seeded, interleaved paired comparisons, with one warmup per worker. Condition ratios are medians of paired process-median ratios; aggregate ratios are geometric means across conditions.

All Oriole builds use O3, ThinLTO and one codegen unit. The Expat controls use GCC O3 without LTO. Each candidate has a fresh profile trained on the original generated corpus, with 288 matching parses in each of the normal, instrumented and optimized builds. Real-project XML remains outside training. Compiler and allocator experiments are not composed into these candidates.

Across six mode/candidate campaigns, all 4,032 workers completed and all 637,056 samples were retained. Timing workers ran on CPU 0 after the build and functional targets completed. Independent saved-data reviews verified the original records, bindings, callback hashes, ordering and arithmetic. The data does not establish a complete explanation for the regressions or a result on other hardware.

## Recompute the saved results

```console
python3 verify.py > recomputed.json
```

The portable reader checks the archive index and all member hashes, verifies each source snapshot, reconstructs every worker's samples, warmups, callback observations and seeded order, then recomputes all condition and aggregate ratios. It does not extract or execute archived files. The same command works after copying this directory elsewhere.

[evidence.tar.gz](evidence.tar.gz) contains the original native records and controllers, source snapshots and patches, build reports and logs, local checks and independent reviews. [files.json](files.json) indexes its 4,460 members. Compiled libraries, compiler installations and build caches are represented by their recorded identities rather than copied into Git. Consequently, this portable check verifies saved evidence; it does not independently rebuild those artifacts or rerun the parsers. Historical helper paths inside the archive describe the original machine and are not portable execution instructions.

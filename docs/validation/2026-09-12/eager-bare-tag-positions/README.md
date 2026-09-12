# Eager positions for bare ASCII opening tags

We retain this bounded optimization as an incremental improvement. Confirmation observes 2.10% less native real-input time and 0.40% less CPython time against the preceding attribute runtime. The performance goal remains open: confirmation is 1.3581× Expat natively and 1.1510× through CPython, above roughly 1.10×. The small Python gains are observations, not a statistical-significance claim.

## Source and behavior

The candidate is runtime `19251bc01dba14cc57507facd2d607a9075471b5`, based on PR159’s `0c407f28e2aa01178fda86a97561ed734f8d8e1d`, which publishes unchanged attribute-scanner control `910387239a804a038feb0a184b1bcfc0a7dd60bf`. Only `tag.rs`, `encoding.rs` and core `lib.rs` change among 74 main files; four auxiliary files and dependencies are unchanged.

The existing opening-name walk records whether every accepted name character is ASCII. An immediate `>` or `/>` proves a nonempty tag has no newline. After the original semantic parsing, raw publication and accounting check, unanchored native UTF-8 input without conversions can advance its eager column without rescanning that tag. Existing cursor, retirement and compaction work remains. Unicode, whitespace, attributes and fallback paths keep their original coordinate scan; Rust and C positions remain eager. Closing tags and implicit empty-tag End positions are unchanged.

The proof bookkeeping also runs on opening names that later do not qualify. Saved actual codegen shows a 100-byte standalone `consume_ascii_tag` helper, whose native branch has no character loop; a converted-length branch remains outside the caller’s eligible path. `TagScanner::scan` changes from 3,638 to 3,491 machine-code bytes and retains per-character proof-field stores. These are code observations, not object-layout measurements, instruction counts, measured frequencies or explanations of elapsed gains. The saved captures do not include the candidate Parser parent or control `name_end` body.

The separate local-proof follow-up accumulates the same flag locally before saving it. Its source and peer review are retained under `future-unbuilt/`; it has not been formatted, compiled, tested or measured and is absent from the selected source. Whether a compiler sinks its stores or spills differently remains unknown.

## Separate measurements

Ratios below are geometric means of the condition-level median paired ratios. Lower is faster; “improved” compares the candidate with its immediately preceding attribute control. Initial and confirmation processes, samples and medians remain separate.

| Epoch | Native group | Conditions improved | Oriole / control | Oriole / Expat |
| --- | --- | ---: | ---: | ---: |
| Initial | real | 24/24 | 0.977613× | 1.354543× |
| Initial | generated | 2/4 | 0.963126× | 3.766672× |
| Initial | real namespaces off | 12/12 | 0.976247× | 1.302549× |
| Initial | real namespaces on | 12/12 | 0.978980× | 1.408612× |
| Confirmation | real | 23/24 | 0.978963× | 1.358065× |
| Confirmation | generated | 2/4 | 0.960195× | 3.747264× |
| Confirmation | real namespaces off | 11/12 | 0.979332× | 1.306587× |
| Confirmation | real namespaces on | 12/12 | 0.978595× | 1.411572× |

| Epoch | CPython consumer | Conditions improved | Oriole / control | Oriole / Expat | Within 1.10× Expat |
| --- | --- | ---: | ---: | ---: | ---: |
| Initial | all | 17/24 | 0.994992× | 1.145892× | 5/24 |
| Initial | elementtree | 8/12 | 0.995116× | 1.196225× | 3/12 |
| Initial | pyexpat-events | 9/12 | 0.994868× | 1.097678× | 2/12 |
| Confirmation | all | 16/24 | 0.995989× | 1.150972× | 5/24 |
| Confirmation | elementtree | 8/12 | 0.995548× | 1.204521× | 3/12 |
| Confirmation | pyexpat-events | 8/12 | 0.996431× | 1.099804× | 2/12 |

Confirmation pyexpat is 1.0998× Expat in aggregate; ElementTree remains 1.2045×. Five of all 24 Python conditions meet the 1.10× threshold. This does not establish that an arbitrary project or production workload meets it.

## Every adverse condition

All 20 rows below have median candidate/control time above one. They remain in the full CSV even where the generated or real-input aggregate improves.

| Epoch | Consumer | Input | Feed | Mode | Oriole / control |
| --- | --- | --- | ---: | --- | ---: |
| Initial | native | Rare declarations | 4 KiB | namespaces off | 1.016885× |
| Initial | native | Rare declarations | 64 KiB | namespaces off | 1.024387× |
| Initial | CPython | Vulkan | 4 KiB | elementtree | 1.001585× |
| Initial | CPython | Vulkan | 64 KiB | elementtree | 1.012382× |
| Initial | CPython | Batik | 4 KiB | pyexpat-events | 1.009640× |
| Initial | CPython | Batik | 64 KiB | elementtree | 1.011336× |
| Initial | CPython | Batik | 64 KiB | pyexpat-events | 1.010717× |
| Initial | CPython | DocBook | 4 KiB | elementtree | 1.002694× |
| Initial | CPython | DocBook | 4 KiB | pyexpat-events | 1.000419× |
| Confirmation | native | Batik | 4 KiB | namespaces off | 1.002286× |
| Confirmation | native | Rare declarations | 4 KiB | namespaces off | 1.023800× |
| Confirmation | native | Rare declarations | 64 KiB | namespaces off | 1.012013× |
| Confirmation | CPython | Vulkan | 4 KiB | pyexpat-events | 1.006914× |
| Confirmation | CPython | Maven | 64 KiB | elementtree | 1.004181× |
| Confirmation | CPython | Batik | 4 KiB | elementtree | 1.005432× |
| Confirmation | CPython | Batik | 4 KiB | pyexpat-events | 1.017144× |
| Confirmation | CPython | Batik | 64 KiB | elementtree | 1.005402× |
| Confirmation | CPython | Batik | 64 KiB | pyexpat-events | 1.009239× |
| Confirmation | CPython | DocBook | 64 KiB | elementtree | 1.010657× |
| Confirmation | CPython | DocBook | 64 KiB | pyexpat-events | 1.005741× |

## Method and host scope

Normal native runs use 28 conditions (24 original-project conditions plus four generated controls), seven matched pairs, 84 preflight workers and 588 timed workers per epoch: 672 workers and 106,176 raw samples. CPython uses 24 conditions, seven matched pairs, 72 preflight workers and 504 timed workers per epoch: 576 workers and 26,364 samples. Across four epochs the packet retains 104 conditions, 2,496 workers and 265,080 samples. Warmups and every measured sample remain in the raw records; each saved reader reconstructs the original exclusions and medians.

The native C driver retains its original create/register/feed/callback/free measurement boundary. The initial CPython epoch runs the unmodified 3.12.13 pyexpat and ElementTree consumers, reusing the exact four control/reference modules and building two fresh candidate modules at O2. Confirmation reuses all six modules and performs no compilation. Callback, canonical-result and loaded-module identities are checked before timing. Strict correctness modules use the explicit cleanup backport and are separate from benchmark modules.

The six-project XML corpus and both feed widths (4 KiB and 64 KiB) are pinned. Native covers namespaces off/on; CPython covers ElementTree and pyexpat event consumers. The README table selects only confirmation 4 KiB/namespaces-off native rows and uses medians of seven process medians; ratios are independently medians of seven paired ratios, so a ratio need not equal the quotient of displayed times. Its derivation pins all 126 worker inputs.

Oriole uses generic normal O3/ThinLTO/one codegen unit with local Ohm rustc 1.98.1-dev and experimental defaults disabled. Expat 2.8.4 uses GCC 13.3 O3 without LTO. The host is Linux AMD EPYC-Milan; elapsed workers use CPU0. No new PGO, CPU-target, allocator or dependency settings are measured.

Only fully interior consecutive host-counter intervals are attributed to each elapsed window. Idle includes iowait. Pre-run observations remain separate, including both initial Python pre-run observations. No interpolation or per-condition host attribution is used.

| Epoch | Consumer | Interior seconds / elapsed seconds | Other CPUs idle + iowait |
| --- | --- | ---: | ---: |
| Initial | native | 90.022 / 95.466 | 94.09% |
| Initial | CPython | 205.061 / 208.781 | 97.81% |
| Confirmation | native | 90.022 / 94.941 | 94.05% |
| Confirmation | CPython | 205.061 / 210.579 | 98.20% |

This is a shared host. CPU affinity, this task’s lack of concurrent parser/compiler work and the counter observations do not establish global isolation or statistical significance. Both rounds show a native aggregate gain; the 0.40–0.50% Python observations remain small.

## Compatibility and review

The exact candidate passes 459 Rust tests across 35 groups, formatting, strict workspace Clippy and locked benchmark Clippy. Focused tests exercise split/resumed proof state, undefined-prefix error order, eager positions, prior-CR handling and compaction boundaries. The complete build/source/library receipts and actual compiler vectors are retained.

The original API matrix preserves all 4,740 ordered outcomes: 4,347 passes, 391 assertion failures and two timeouts, raw exit 1. Original assertions, allocation retry ceilings and 3-second/1 GiB address-space/768 MiB resident-memory case bounds remain unchanged. Six shared/static C integration, adversarial and allocation consumers pass. Their ASan/UBSan instrumentation covers C harnesses; Rust normal-release libraries are uninstrumented and leak detection is disabled.

Shared and static strict CPython each retain 802 method outcomes and 809 rendered lines, with the same two callback-grouping failures (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`) and raw exit 2. These consumers include the explicit pinned pyexpat cleanup backport. Two separate supplemental semantic tests pass per linkage; those results do not reclassify strict failures. Current-source sustained fuzzing and an installed PBS distribution trial remain outstanding.

The packet author also prepared the runtime and its ordinary/native/Python/correctness protocols. Separate peers reviewed source/composition and the controller/readers; root owned all target execution, and separate reviewers reconstructed the independent native/Python results from saved records. Saved reconstruction is independent of target execution, with runtime/preparation authorship disclosed. The final packet reader has a separate author. Exact source maps retain candidate19251bc and control910387, each with 74 main and four auxiliary files; unchanged dependency archives and all 156 source/license files remain available.

Original source-authoring, composition, formatting and saved-reader preparation attempts retain their own scopes. The first composition check grouped shared braces differently; its revised reader used exact transformations. Source-review setup corrections did not rerun parser targets or change upstream assertions. Original held drafts are archived separately. The completed gate receipts determine actual outcomes; expected counts are not substituted for missing results.

The preceding attribute and rejected compact-owner campaigns remain in their [source-specific report](../attribute-lane-masks/). They are not extra epochs in this packet. The separate local-proof follow-up remains unbuilt; the performance and production-readiness goals remain open.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 5,806 indexed files (10,674,807 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 104 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 20 adverse rows. Four campaigns remain separate; no observations or medians are pooled. [report.json](report.json) retains full precision.

Archive SHA-256: `d617722505cfca1363e95a4df60f9e368ba05e03266431fb72e5106ed6f28e0e`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.

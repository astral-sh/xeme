# Expanded namespace Start frames

The capacity refinement (V2) reduces normal native parsing time by **3.40%** and actual CPython parsing time by **2.58%** against selected runtime `4064b065`. The resulting aggregates remain **1.5424× Expat natively** and **1.2396× through CPython**, above the roughly 1.10× target. Original API and strict CPython outcomes are unchanged, including the remaining failures.

Measured source bytes are committed at `edc3e61eddeac27deaea1d76bf3a2f6a892c1c25`, rebased onto the manual-PGO-workflow publication parent. The [report](report.json) includes the exact 73-file commit binding. V1 is preserved at `8c039ec6c24797c516312a277d0e102c0768f077`. This packet records normal builds only; no PGO build or training experiment was performed.

## Change and refinement

V1 lowers eligible native namespace Start callbacks directly into a detached frame, avoiding generic owned Start/attribute reconstruction. Eligibility keeps unprefixed ordinary attributes, existing namespace/error/resource behavior, the 4KiB arena bound and the 128-attribute bound. Expanded and raw names retain the packed stack layout; callbacks receive separately owned frame bytes, with no parser reference spanning a callback.

V1 improves Maven and DocBook namespace conditions, but adds allocation requests when nested prefixed names outgrow their freshly expanded name owner. V2 keeps the existing owner when its capacity fits; otherwise it packs into the other existing cached owner and recycles the expansion temporary. Generic expand_name and URI charging are unchanged, and default-namespace suffix names keep the move path. The exceptional branch makes a third expanded-name copy. No persistent field or additional cache was added.

## Normal performance

Linux x86-64 AMD EPYC-Milan host. Oriole uses Ohm rustc 1.98.1-dev (`f62703110`, LLVM 22.1.8), O3, ThinLTO and one codegen unit. Expat 2.8.4 uses GCC 13.3 O3 without LTO. CPython 3.12.13 pyexpat and _elementtree sources are unmodified and compiled identically with GCC 13.3 O2; only parser linkage differs.

Six original project XML inputs (Vulkan, Wayland, Maven, Batik, GTK, DocBook), 4KiB and 64KiB feeds. Native adds namespaces off/on and four generated control conditions; Python uses ElementTree and pyexpat. These are parser workloads on project XML, not complete project executions. Each version has its own seven seeded paired cohorts against the same selected normal Oriole and normal Expat libraries. **There is no directly paired V2-versus-V1 elapsed comparison.**

| Version and group | Time / selected | Time / Expat | Faster than selected |
| --- | ---: | ---: | ---: |
| V1 native real | 0.980018× | 1.565630× | 9/24 |
| V1 native generated | 0.978290× | 4.001900× | 4/4 |
| V1 Python all | 0.979029× | 1.241269× | 13/24 |
| V1 ElementTree | 0.972020× | 1.319333× | 8/12 |
| V1 pyexpat | 0.986088× | 1.167824× | 5/12 |
| V2 native real | 0.966009× | 1.542442× | 22/24 |
| V2 native generated | 0.962493× | 3.893823× | 2/4 |
| V2 Python all | 0.974195× | 1.239639× | 16/24 |
| V2 ElementTree | 0.970247× | 1.313460× | 7/12 |
| V2 pyexpat | 0.978160× | 1.169966× | 9/12 |

Ratios are medians of seven paired process-median ratios, then equally weighted geometric means across conditions. Each version retains 672 native workers (84 preflight, 588 timed; 105420 measured samples) and 576 Python workers (72 preflight, 504 timed; 25788 measured samples). Timed workers discard one warmup. All 2496 workers and 265080 total samples are retained across both versions.

Native parsing includes its original callbacks. Python timing includes parser construction, feeds, finalization, callbacks and explicit result destruction; input reads/imports/canonical checks/gc.collect are outside timing, and automatic GC remains enabled. Native hashes and Python canonical checks merge adjacent text fragments, so they do not establish exact callback fragmentation. Shared-host frequency/cache/bandwidth remain uncontrolled; one campaign per version does not establish statistical significance.

All 104 conditions are in [conditions.csv](conditions.csv), including every pair in the report and raw records. All **38 adverse conditions** are in [adverse-conditions.csv](adverse-conditions.csv): V1 has 15 native-real and 11 Python regressions; V2 has two native-real, two generated and eight Python regressions. Four of 24 V2 Python conditions fall within 1.10× Expat.

V2 adverse conditions:

| Consumer | Project | Feed | Mode | Time change |
| --- | --- | ---: | --- | ---: |
| native | batik | 4KiB | namespaces on | +0.889% |
| native | gtk | 4KiB | namespaces on | +0.246% |
| native | generated-rare-declarations | 4KiB | namespaces off | +0.333% |
| native | generated-rare-declarations | 64KiB | namespaces off | +0.926% |
| Python | vulkan | 4KiB | elementtree | +1.673% |
| Python | vulkan | 4KiB | pyexpat-events | +0.378% |
| Python | vulkan | 64KiB | elementtree | +0.732% |
| Python | vulkan | 64KiB | pyexpat-events | +1.175% |
| Python | wayland | 4KiB | pyexpat-events | +0.354% |
| Python | batik | 4KiB | elementtree | +0.048% |
| Python | gtk | 4KiB | elementtree | +0.377% |
| Python | gtk | 64KiB | elementtree | +0.802% |

## Supplied allocator observations

The unchanged probe uses eight inputs per library, cold feed boundaries, and the actual selected allocation suite. Both versions match callback counts/hashes, API origins and complete input consumption; all 32 raw probes end with zero live blocks after parser free. The repeated selected-normal observations match exactly.

| Case | V1 phase-2 request delta | V2 phase-2 request delta | V1 peak byte delta | V2 peak byte delta |
| --- | ---: | ---: | ---: | ---: |
| maven-off | +0 | +0 | +0 | +0 |
| maven-pipe | -301 | -301 | +3 | +3 |
| flat-pipe | +1 | +1 | +12 | +64 |
| empty-pipe | +1 | +1 | +12 | +64 |
| nested-pipe | +196 | +1 | +202 | +59 |
| equal-nul | +1 | +1 | +4 | +4 |
| triplets-pipe | +1 | +1 | -6 | +48 |
| capacity-2044 | +15 | +15 | +0 | +0 |

All deltas are against selected 4064. V2 removes 195 of V1’s 196 extra nested requests, while Maven keeps its 301-request reduction. Flat/empty and triplet peaks increase versus V1 because the expansion temporary stays cached. The 2044-byte suffix case still makes 15 additional frame allocations at post-warmup feed boundaries. [allocations.csv](allocations.csv) retains requested bytes and final retention; the report and raw histograms retain every phase and per-feed observation. This probe measures per-phase requests and per-feed live bytes/blocks, not RSS or allocation counts separately for each feed.

## Correctness

- V1 and V2 each pass 442 workspace tests, strict all-target Clippy and formatting. The check reports have 35/34 groups respectively because V2 omits the unchanged zero-test CLI group. Focused regressions cover nested packed names, UTF-8/separators, error and work-limit ordering, allocation failures, callback pointers and reentry. Independent source reviews found no blocker.
- The original API matrix preserves all 4740 ordered results: **4347 pass, 391 assertion failures, two timeouts**. The raw matrix process exits 1. Assertions and resource limits remain unchanged. Six C consumers pass with ASan/UBSan on the C harnesses; the linked Rust normal-release libraries are uninstrumented, and leak detection is disabled.
- Shared/static strict CPython each preserve **802 distinct method outcomes, two failures, raw exit 2**. The exact baseline maps and all module/parser origins match. There are 809 rendered outcome lines: 802 distinct methods, six extra C14N subtest outcomes and one skipped class setup; 14 continuation descriptions replace identity lines and do not add outcomes. Strict consumers include only the explicit upstream pyexpat allocation-failure cleanup backport and remain separate from unmodified benchmark extensions.
- Both existing supplemental semantic tests pass for each linkage. They check text preservation, callback-controlled buffering and element/CDATA ordering without changing strict assertions or reclassifying those failures.

Earlier allocation retry/version/resource-policy differences and the two strict callback-grouping failures remain. These gates do not make the original upstream suites green or establish general production readiness.

## Diagnostic and provenance limits

Four Callgrind profiles belong only to selected normal source 4064: Maven/GTK at 64KiB, namespaces off/on. They include callbacks inside XML_Parse and exclude construction/free outside it. Maven’s recorded instruction count rises from 14709923 to 20321505 with namespaces; GTK rises from 6266840 to 6464736. Required expanded callback work is included, so these are not removable-work or elapsed predictions. **No V1 or V2 instruction profile is claimed.**

Initial focused-test, source-count/binder, wrong-target-directory and strict-preparation attempts remain in the archive alongside their corrections. Root ran target campaigns; a separate reviewer checked runtime sources and independently reconstructed native/compatibility records. Python build/origin/sample arithmetic was independently audited. The core author prepared the supplied-allocator probe and its V1 saved readback; that is distinct from independent source review.

Previous 4064 [optimized measurements](../../2026-09-11/reference-frames/), [optional v3 results](../../2026-09-11/reference-v3/), [sustained sanitizer/PBS validation](../../2026-09-11/reference-validation/) and [installed v3/glibc trial](../v3-distribution/) retain their historical source scope. This packet claims no new-source CI, sustained fuzzing, installed PBS/glibc or optimized performance result.

## Reproduce the evidence readback

The [archive](evidence.tar.gz) contains 5,775 files (9,797,502 compressed bytes); executables, libraries and compiler profile data are excluded. [index.json](index.json) maps every member to its original path, byte size and SHA-256. Extraction hashes and every member were read back after writing. The [machine-readable report](report.json) retains all groups, ratios, origins and compatibility outcomes.

```console
sha256sum evidence.tar.gz
tar -xzf evidence.tar.gz
```

Archive SHA-256: `454c89ce5dba42de97d6b7806a38c4819bb888bda9e2725d1bbbb77846dcc827`. Use the member hashes in `index.json` to verify extracted files; recorded absolute paths describe original provenance.

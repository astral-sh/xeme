# Matched native End raw ranges — draft qualification

**Qualification is in progress. This runtime is unselected.** This packet supports a draft review and remote Miri checks; it does not mark the candidate ready for production. Current-source Rust ASan/fuzz and both Miri aliasing models remain pending. Local Ohm lacks Miri. No installed CPython-distribution qualification or installed-speed result is claimed for this candidate.

## Change and tested identity

For an exact matched native UTF-8 End in the eligible C input-context path, the parser records a range in its existing owned input instead of copying the End token into scratch and owned raw-markup storage. It detaches the same owned element name for the callback, preserving accounting, eager positions and callback ownership. Generic, converted, namespace-undo and other fallback paths retain their token copies. Allocation schedules and later buffer reuse may change. This is separate from every Start-token candidate.

The tested [three-file patch](source/candidate.patch) is 215 additions and 17 deletions against base `008d818237fe0d6f8c04a000a98ad69ab5780ceb`; only core `lib.rs` changes production behavior. [Source and build binding](source/build-binding.json), [complete source map](source/source.json), and [independent source review](source/independent-source-review.json) identify the exact tested bytes. The measured snapshot based on `008d818237fe0d6f8c04a000a98ad69ab5780ceb` maps to source commit `410f7608405f9ffd46697fa380b120b86496f771`, rebased after the installed-results follow-up. The [publication source binding](source/publication-source.json) verifies that all 78 compiled source and auxiliary files remain exact after committing and rebasing; measurements preceded this commit. The commit also includes a CI-only check extension outside the measured source patch, with no completed result recorded here. The runtime remains unselected.

| Library | SHA-256 |
| --- | --- |
| Candidate shared | `5ca9644b5d782a6beaae7a8b1ddd16045480d4566b7eb77e317c7ac25a2258fe` |
| Candidate static | `0e03a6aef524eb621b2ef67983303e8e6d4f45eac19cbf0e8f21c08c829ebfed` |
| Selected raw-view shared control | `e59d89d6e21b92042b8358bd8ceef45b0a60404c61c9aa06ccf2e85a00f1c7f6` |
| Normal Expat shared control | `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478` |

The selected control was compiled from runtime `67c704c123b8661a3ad1f91bd48c2f0be4996096`. Base 008 includes a later [three-line cfg(test) userdata correction](source/control-fixture-difference.patch); the control was not rebuilt. The build binding records three changed files versus 008 and four versus the compiled control. The ordinary build passed **476 tests across 35 result groups**, plus all seven prescribed check/build commands.

## Measurements, including adverse cases

Ratios are candidate elapsed time divided by the indicated control; below 1 is less time. Each condition uses the median of seven paired process-median ratios; aggregates are equally weighted geometric means of condition ratios. Initial and confirmation campaigns remain separate. All 28 native and 24 Python conditions per epoch are retained: **104 condition observations, 2,496 workers and 265,080 samples including preflight/warmups**.

| Scope | Initial / selected | Confirmation / selected | Confirmation / Expat | Conditions lower than selected, initial; confirmation |
| --- | ---: | ---: | ---: | ---: |
| Native real projects | 0.984143 | 0.987696 | 1.254649 | 19/24; 17/24 |
| Native generated controls | 1.013153 | 1.013491 | 3.461543 | 1/4; 0/4 |
| Python, both consumers | 0.991017 | 0.989399 | 1.086107 | 17/24; 20/24 |
| ElementTree | 0.989974 | 0.984715 | 1.125254 | 8/12; 11/12 |
| pyexpat events | 0.992062 | 0.994105 | 1.048321 | 9/12; 9/12 |

The project performance target now permits up to 20% additional elapsed time versus Expat. The confirmation native real-project aggregate is **25.46% above Expat**, so it still misses that target. The ElementTree and pyexpat aggregate comparisons are 12.53% and 4.83% above Expat. These aggregate results do not waive individual outliers or compatibility failures. Historical readbacks retain their original 10% fields unchanged.

Vulkan regresses in three initial conditions and all four confirmation conditions; the fourth changes from an improvement to a regression. Generated controls regress 1.32% initially and 1.35% in confirmation; all four generated confirmation conditions are adverse. These are retained tradeoffs, not excluded samples.

| Vulkan input chunk | Namespaces | Initial / selected | Confirmation / selected |
| --- | --- | ---: | ---: |
| 4 KiB | off | 1.046024 | 1.053232 |
| 4 KiB | on | 1.032325 | 1.032387 |
| 64 KiB | off | 0.977382 | 1.025444 |
| 64 KiB | on | 1.054213 | 1.043695 |

Complete condition tables and all adverse rows: [initial native CSV](initial/native/conditions.csv), [native readback](initial/native/readback.json), [confirmation native CSV](confirmation/native/conditions.csv), [native confirmation readback](confirmation/native/readback.json), [initial Python CSV](initial/python/conditions.csv), [Python readback](initial/python/readback.json), [confirmation Python CSV](confirmation/python/conditions.csv), [Python confirmation readback](confirmation/python/readback.json). The [earlier decision](qualification-decision-historical.json) advances qualification only; its pending-check list is a historical checkpoint.

### Method and limits

Measurements used a shared Linux x86-64 host with elapsed work pinned to CPU 0. Oriole uses ordinary generic x86-64 Ohm rustc 1.98.1-dev, O3/ThinLTO/codegen-units 1; normal Expat 2.8.4 is the unchanged control. There is no PGO or CPU-specific target tuning. CPython 3.12.13 benchmark consumers are unmodified, built at O2; [consumer binding](source/python-consumer-binding.json), [control reuse](source/python-control-reuse.json), and [compiler script](source/build_python_consumers.py) record their origins. These runs parse project XML inputs, not whole applications. Host observations for native timing ([initial before](initial/host-before-native.json), [initial during](initial/host-during-native.json), [confirmation before](confirmation/host-before-native.json), [confirmation during](confirmation/host-during-native.json)) do not establish CPU isolation or statistical significance. Callback checks coalesce adjacent text; strict callback fragmentation is checked separately below.

## Compatibility evidence and remaining checks

| Check | Completed result and limit |
| --- | --- |
| Original upstream API | **4,349 pass / 391 fail / 0 timeout**, all 4,740 ordered outcomes identical to selected raw-view; suite exit 1. Original assertions and 3 s / 1 GiB / 768 MiB limits retained. The two 2 GiB timeout improvements predate this candidate. [API/C readback](qualification/api-c-readback.json) |
| Six C consumers | Integration, adversarial and allocation programs pass with shared and static linkage. C harnesses use ASan/UBSan with leak detection disabled; the linked Rust libraries are normal and uninstrumented. This is not Rust ASan qualification. [Commands](qualification/commands.json) |
| Strict CPython | Each linkage retains 802 methods / 809 rendered outcomes, exit 2 and the same two failures: `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. No relaxed test assertions. Only the documented upstream pyexpat cleanup backport is applied to strict consumers; benchmark consumers are unmodified. [Strict readback](qualification/strict-semantic-readback.json), [consumer patch identity](qualification/strict-preparation.json) |
| Supplemental text semantics | Both original semantic tests pass for each linkage; strict failures remain failures. [Shared receipt](qualification/semantic-shared-receipt.json), [static receipt](qualification/semantic-static-receipt.json) |
| Allocation retry | All 300 configurations pass with 25 test-local maxima raised to 512; semantic assertions remain unchanged. Stops at first success, so this is not exhaustive OOM testing and does not reclassify upstream failures. [Exact ceiling edits](qualification/allocation-retry/retry-ceilings.patch), [readback](qualification/allocation-retry/saved-readback.json) |
| Buffer tail | All 12 configurations pass. Exactly two allocation-result assertions are observations; every remaining tail assertion is unchanged. [Exact diagnostic edits](qualification/buffer-tail/diagnostic.patch), [readback](qualification/buffer-tail/saved-readback.json) |
| W3C | Each engine has 4,962 mandatory passes, **960 mandatory failures**, 81 optional observations and 0 inconclusive results over 6,003 rows. All candidate/selected rows are identical; candidate/Expat acceptance agrees, but 225 error, 1,695 byte-index and 21 child fields differ. Acceptance equality is not conformance or callback equality. [W3C readback](qualification/w3c-saved-readback.json), [supplemental comparison and limitations](qualification/supplemental-review.json) |
| Pending | Remote Miri under both aliasing models, current-source Rust ASan/fuzz, final release review and runtime selection. Neither earlier-source qualification nor this draft substitutes for these checks. |

## Reproduction and retained local evidence

The [copy index](COPY_INDEX.json) records exact bytes and local origins. Controllers/readers are copied beside each epoch's reports; native protocol manifests preserve every input/library/driver hash and Python scripts preserve worker behavior. [Build controller](source/build.py) and [binder](source/bind_build.py) retain exact ordinary compiler commands. These are archived recipes with their original absolute local paths, not relocatable turnkey commands.

[LOCAL_RAW_INDEX.json](LOCAL_RAW_INDEX.json) identifies retained local results/preflight files by the hashes verified by the completed saved readers. Native results contain complete worker command/exit/stdout records; Python readbacks additionally preserve individual raw-worker evidence hashes. Full API, strict and W3C raw files remain at the paths and hashes in their readbacks. The packet does not duplicate thousands of workers, corpus files, compiler outputs or binaries. Historical preparation statuses and 10% thresholds in copied receipts are preserved as historical metadata; the current status is this draft's qualification-in-progress statement.

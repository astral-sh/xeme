# Outer whitespace separated dispatch

Runtime `6320d7b75a660866cb701cf2f50a1ec61084fbf5`, stacked on PR162 `87acc5a91ad3b29b04a9a902cd8bdeebed00529f`. We preserve the original inner-content plan and add a separate transient native space/TAB proof outside the root. The proof stops at the original first boundary and gives up entirely on uncertain input, preserving fallback, callback/error precedence, eager coordinates and resource accounting. Four files differ; ordinary checks passed 466 Rust tests in 35 groups.

## Substantive API change

Both original chunk-0 `test_misc_input_2gb` cases (deferral 0/1) passed in the targeted candidate probe and full matrix under the unchanged three-second alarm; selected control timed out 14. Full API readback preserves all 4,740 rows and changes only ordered rows 110/505 from timeout 14 to pass 0: 4,349 passes /391 failures /zero timeouts. Six C consumers pass. Independent full correctness readback passed. Strict shared/static CPython each retain 802 methods, 809 rendered outcomes and the same two grouping failures (raw exit 2); both separate semantic tests pass per linkage. No failure reclassification or raised ceiling belongs to this claim.

## Four separate timing epochs

| Epoch | Consumer | Candidate/control | Candidate/Expat | Scope |
| --- | --- | ---: | ---: | --- |
| Initial | Native,24 real |0.998987805|1.294736586|Completed primary/independent records |
| Confirmation | Native,24 real |0.999574572|1.295048228|12 adverse across all28 conditions; independently read back |
| Initial | CPython,24 |0.994636448|1.121234780|17 wins /7 adverse |
| Confirmation | CPython,24 |0.992608945|1.118172263|19 wins /5 adverse |

Native timing is roughly tied. Python elapsed time is 0.54% lower initially and 0.74% lower in confirmation; neither is a statistical-significance claim. All real/generated native rows, all Python rows and every adverse row remain in each separate campaign. Four main epochs total 104 rows, 2,496 workers and 265,080 samples. No pooled medians or favorable-only table. The confirmation pyexpat aggregate is 1.0767× Expat, ElementTree is 1.1612×, and five individual Python conditions meet the roughly 1.10× goal; both overall aggregates remain above it. The candidate is selected for the two original timeout fixes, with native timing roughly tied and modest Python gains. All 35 adverse rows remain visible across four separate epochs. The roughly 1.10× overall performance goal remains open, and an installed PBS trial of this source remains outstanding.

## Method and compatibility scope

The inherited protocol uses six pinned project XML inputs,4/64KiB chunks, both native namespace modes and unchanged CPython3.12.13 ElementTree/pyexpat consumers. Seven paired orders, warmups, iterations, canonical callback/output checks, enabled GC and explicit destruction remain unchanged. Oriole normal artifacts use generic O3/ThinLTO/cgu1 with Ohm1.98.1-dev; normal Expat2.8.4 uses GCC13.3 O3 without LTO. README times are medians of seven process medians; displayed ratios are medians of paired ratios. The initial Python epoch built two candidate modules and reused four; confirmation reuses all six with zero compiles. CPU0 affinity and before/during idle+iowait observations do not establish exclusive-host operation. Keep every observation, fully interior counter interval and boundary exclusion with its own epoch.

Strict CPython retains 802 methods /809 rendered outcomes and the same two grouping failures (raw exit 2) per shared/static linkage; two supplemental semantic tests per linkage remain separate. Six C harnesses retain C-only ASan/UBSan against uninstrumented normal Rust, with leak detection disabled. Independent original API/C and strict/semantic readback passed. Separate diagnostics pass all 300 retry configurations after raising exactly 25 local retry maxima to 512, and all 12 buffer-continuation configurations after replacing exactly two second-empty-call allocation-result assertions with unconstrained observations. Remaining semantic/tail assertions stay unchanged. Retry probes use 15-second tests, 4 GiB address space and 3 GiB RSS limits; buffer probes retain three seconds, 1 GiB and 768 MiB. These uninstrumented consumers do not fix the original 391 failures or establish exhaustive OOM coverage; the buffer fixture is not a callback-text bytes/count oracle. Current-source Rust ASan passes all six harnesses: 69,199 replay executions and 8,194,533 exploration executions. Nine required Rust crate/target invocations are ASan-instrumented; normal generic libraries are provenance controls only. Each harness uses a 600-second exploration bound, a ten-second input timeout, 1,536 MiB RSS limit and 65,536-byte maximum input, with leak detection disabled. All 22 recorded commands exited zero and their children were reaped. This bounded campaign does not establish exhaustive memory safety or installed-distribution compatibility. The first launch failed before fuzz compilation because its PATH omitted cargo/bin, so cargo-fuzz could not invoke bare rustc; all child processes were reaped and all six harnesses remained unstarted. The retry changes only the output path and fixes the launch PATH, retaining the runner, source, target directory, flags, six harnesses and bounds. No installed PBS or production-readiness claim is implied.

## Evidence boundaries

Archive only current four-campaign raw records, source maps of 74 main and four auxiliary files for selected/candidate, compiler vectors, original upstream API/C/strict raw source/results, targeted two-case probe, current controller/readers and finite supplemental/ASan manifests/logs chosen at final release. Dependencies are unchanged; the existing 156-source/license map and three checksum archives remain bounded inputs. Historical validation reports stay linked in place. Do not copy old benchmark archives, unrelated rejected experiments or inherited ASan corpus/cache trees. Root’s final release binds all completed receipts and preserves any source-only attempts; root alone assembles and installs the six packet files plus four entry docs.

## ASan retry provenance

The first launch failed before fuzz compilation because its PATH omitted cargo/bin, so cargo-fuzz could not invoke bare rustc; all child processes were reaped and all six harnesses remained unstarted. The retry changes only the output path and fixes the launch PATH, retaining the runner, source, target directory, flags, six harnesses and bounds.

The first failed report, build error, version logs and runner remain separate from the successful retry report, review, compiler proof, source manifests and 22 command logs. Six initial corpus manifests and final corpus identities are retained; corpus files, sanitizer binaries and caches are excluded from this packet. The completed scope review owns corpus-byte reconciliation.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 6,353 indexed files (19,594,817 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 104 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 35 adverse rows. Four main campaigns remain separate; no observations or medians are pooled. Targeted API and separately scoped ASan/supplemental evidence are retained outside these timing aggregates. [report.json](report.json) retains full precision.

Archive SHA-256: `1f58ac534015d62cec79ccd4c7e4bfa3d6de017bdd3eb4318c755ca8d8a5ad50`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.

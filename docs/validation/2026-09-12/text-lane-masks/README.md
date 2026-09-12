# ASCII Text lane masks

Retain `cacc0b1b3766176e9cbf71d25c5ff6867fa87c08`: two separate normal-build epochs show modest native and CPython aggregate reductions against sparse namespace storage, with unchanged compatibility outcomes. Confirmation measures **1.55% less native real-project time and 1.21% less CPython time**. The remaining gaps are **1.4248× and 1.1745× Expat**; this does not meet the overall roughly 1.10× goal or establish production readiness.

## Results

| Campaign | Conditions | Project time / control | Project time / Expat | Adverse rows |
| --- | ---: | ---: | ---: | ---: |
| Rejected fixed16 native | 28 | 0.999700× | 1.445001× | 18 |
| LF initial native | 28 | 0.983484× | 1.423312× | 5 |
| LF initial CPython | 24 | 0.989369× | 1.171519× | 2 |
| LF confirmation native | 28 | 0.984516× | 1.424768× | 3 |
| LF confirmation CPython | 24 | 0.987917× | 1.174454× | 1 |

Native project aggregates contain the 24 real-project conditions; four generated conditions are recorded separately in each native campaign. Their time/control aggregates are 1.032372× for fixed16, 0.968173× for initial LF and 0.965363× for confirmation LF. All **132 conditions, 3,168 workers, 371,256 samples and 29 adverse rows** remain in the packet. Samples include preflight and warmup observations. Epochs are never pooled, and fixed16 versus LF was not a paired timing comparison.

Confirmation improves 22/24 native real-project conditions and 23/24 CPython conditions. Native namespace-disabled and namespace-enabled aggregates are respectively 0.989588× and 0.979470× control. CPython ElementTree is 0.987046× control / 1.234734× Expat; pyexpat events are 0.988790× control / 1.117117× Expat. Four CPython conditions meet the roughly 1.10× Expat target.

The four confirmation regressions remain: Vulkan and DocBook at 64 KiB with native namespaces disabled take 1.000297× and 1.000519× control; generated rare declarations at 4 KiB take 1.010710×; DocBook through pyexpat at 4 KiB takes 1.002666×. The initial and rejected-candidate regressions are retained separately.

All measurements ran on a shared Linux AMD EPYC-Milan host. During fully interior confirmation monitor intervals, other CPUs averaged 95.29% idle plus iowait for native and 98.01% for Python. Both native pre-run observations, the delayed first observation, all during-run counters and interval boundaries are preserved. Affinity, counters and local target scheduling do not establish global isolation, statistical significance or freedom from cache, frequency and memory interference. No per-condition host attribution is inferred.

## Implementation and dependencies

The LF classifier uses a 16-byte stop mask and counts newline bits only before the first delimiter or uncertain byte. It retains the existing eight-byte/scalar tails, 64 KiB cutoff, previous-CR handling, position commit and raw ownership. Focused source tests cover byte lanes, first-stop ordering, newline prefixes, partial input and position equivalence. Actual generic release disassembly is retained, including its constant setup and instruction-count scope; it does not prove why elapsed time changed.

The earlier fixed16 classifier, `ae8e1904aedcb6c286d17e7626be51285e4be50a`, remains rejected. Real-project time was effectively flat, only 9/24 real conditions improved, and generated time regressed. It had its own 454 Rust tests and native measurement, with no downstream Python/API/strict campaign. Its source, initial capture/reader corrections, raw results and rejection decision remain intact.

LF has 74 main source files, five changed main entries and two auxiliary lock changes relative to namespace control. Its runtime commit is source-identical to the measured precommit build; the measured base remains `b6985f0d07e9b3456f36f77eac97cf7bf013a650`. Three source member maps retain namespace control, rejected fixed16 and LF identities while storing equal bytes once. Both benchmark/fuzz manifests and locks are preserved.

The checksum-pinned `wide 1.7.0`, `safe_arch 1.2.0` and `bytemuck 1.25.2` archives and all 156 source files, including licenses, are retained. Main, benchmark and fuzz metadata graphs and resolved features were reviewed: wide defaults are disabled, safe_arch's empty default and bytemuck features are enabled, and bytemuck has no enabled features. Metadata and source review do not establish an executed MSRV test or all-platform validation. Existing predecessor SIMD `memchr` and eight-byte word scans remain separate from this new explicit vector classifier.

Oriole uses ordinary generic O3, ThinLTO and one codegen unit with Ohm rustc 1.98.1-dev and its experimental defaults disabled. Expat 2.8.4 separately uses GCC 13.3 O3 without LTO. No target-CPU flags, allocator substitution or new PGO campaign is involved. Earlier PGO/v3/PBS and whole-Rust sanitizer results retain their historical source scope.

## Compatibility

LF passes 455 Rust tests across 35 groups, formatting, strict workspace Clippy, locked/offline benchmark formatting and Clippy, and normal release checks. All 4,740 original API classifications match the baseline: 4,347 passes, 391 assertion failures and two timeouts, with raw exit 1 and unchanged original assertions and limits. This preserves the known failures rather than declaring the matrix green.

All six dynamic/static C integration, adversarial and allocation consumers pass. Their C code uses ASan/UBSan; linked Rust libraries are ordinary uninstrumented builds and leak detection is disabled. These C checks are not a new sustained whole-Rust sanitizer campaign.

Strict CPython shared/static each retain 802 method outcomes and 809 rendered outcome lines, including the same `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` failures and raw exit 2. Strict modules alone include the explicit pinned upstream pyexpat allocation-failure cleanup backport; benchmark modules use unmodified CPython 3.12.13 source. Two supplemental text/callback semantic tests pass per linkage and do not change the strict failures. Current-source sustained fuzzing, an installed PBS trial and broader platform execution remain outstanding.

## Preparation history

The original correctness finalizer `438a1d77…` was materialized and binder56240 succeeded before a descriptive strict-source label changed. Root later applied a separate metadata correction and successfully rebound the saved inputs. Original and corrected records are retained, including their differing review scopes. All nine execution/read scripts stayed unchanged; this caused no target failure or parser rerun. Other source-review/capture corrections and first attempts remain indexed with their original outcomes.

## Reproduction

Native uses the original 28-condition, seven-pair protocol and seed 2026091003, with identical inputs, callbacks, limits and 672 workers per campaign. Python uses 24 conditions, seven pairs and seed 202609104412, with 576 workers per epoch. Initial LF builds two fresh O2 candidate modules and reuses the selected namespace and Expat modules; confirmation reuses all six modules and executes fresh preflights without compiling. Process creation/loading is outside the timed parse region; parser construction, parsing, callbacks and destruction follow the unchanged workers.

The archive retains source, licenses, actual compiler vectors, binary identities, codegen, protocols, workers, commands, stdout/stderr, preflights, all independent reviews and host observations. Main README medians come from the six confirmation 4 KiB/namespace-disabled conditions: each displayed time is a median of seven process medians; each ratio is the median of seven paired ratios. Those two calculations need not equal the ratio of displayed medians.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 6,549 indexed files (12,509,669 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 132 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 29 adverse rows. Five campaigns remain separate; no observations or medians are pooled. [report.json](report.json) retains full precision.

Archive SHA-256: `ac6564aa66a64f4f2fda67030ec75bc451e34cb77decec1461263d994b89c230`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.

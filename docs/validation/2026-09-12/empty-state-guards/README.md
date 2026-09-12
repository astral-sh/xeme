# Empty-state guards: normal build

Selected runtime `66ca8e0bf7d9e59412c290298d49660f12b8c26a` changes three guards in two files on PR160 `c8411446`. Zero-byte accounting returns before the unchanged nonzero body; an absent parameter reference avoids an owned `take`; an empty default queue avoids its initial drain. Nonzero accounting, atomics, populated branches, callbacks and later drains retain their behavior. Two source maps bind all 74 main and four auxiliary files; dependencies remain unchanged.

## Measurements

| Epoch | Real native / control | Native / Expat | Python / control | Python / Expat |
|---|---:|---:|---:|---:|
| Initial | 0.9930287054 | 1.3472694249 | 0.9895371443 | 1.1334602085 |
| Confirmation | 0.9913705647 | 1.3443341722 | 0.9910148170 | 1.1372401853 |

Confirmation observes 0.86% less real-project native time and 0.90% less CPython time against the eager control. Native improves in 22/24 conditions and Python in 19/24. Both native aggregates remain above the roughly 1.10× Expat goal; the combined Python aggregate does too. The pyexpat aggregate is 1.0936× Expat and ElementTree is 1.1826×; five of 24 Python conditions meet the goal.

Initial native improves in 19/24 real conditions and all four generated conditions (generated ratio 0.9350972744). Its five adverse real rows remain. Initial Python improves in 20/24 conditions, with four adverse rows. Confirmation’s four generated cases all improve (ratio 0.9393145413), while Wayland with namespaces on regresses 0.9059% at 4 KiB and 0.6938% at 64 KiB. Five Python confirmation regressions remain: Wayland pyexpat 4/64 KiB (+0.5645%/+0.1325%), Batik ElementTree 4 KiB (+0.3748%), Batik pyexpat 64 KiB (+0.8529%) and DocBook ElementTree 64 KiB (+0.2135%). All 16 adverse rows across all four epochs are retained.

### Six projects, confirmation 4 KiB/namespaces off

| Project XML | Oriole (normal) | Expat (normal) | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 42.999 ms | 28.767 ms | 1.500× |
| Wayland protocol | 0.996 ms | 1.076 ms | 0.929× |
| Maven POM | 0.710 ms | 0.438 ms | 1.622× |
| Batik SVG | 0.129 ms | 0.134 ms | 0.961× |
| GTK UI | 0.296 ms | 0.216 ms | 1.375× |
| DocBook XSL | 0.247 ms | 0.191 ms | 1.295× |

Times are medians of seven process medians; ratios are independently computed medians of seven paired ratios. The table binds exactly 126 raw candidate/control/Expat workers and does not pool epochs.

The 104 conditions, 2,496 workers and 265,080 samples belong to four separate campaigns. Each retains seven seeded pairs, original worker/count/digest/origin checks and complete adverse rows. Initial Python builds two unmodified CPython candidate extensions with GCC -O2 and reuses four control/Expat extensions. Confirmation reuses all six and performs fresh preflights; it compiles no modules. Native uses the same existing driver. Parser construction, GC/destruction and timing boundaries remain unchanged. Normal Oriole uses local Ohm rustc 1.98.1-dev, generic O3/ThinLTO/codegen-units=1; normal Expat 2.8.4 uses GCC 13.3 O3 without LTO. No PGO work or per-candidate CPU flags occurred.

| Shared-host window | Fully interior other-CPU idle+iowait | Covered elapsed time |
|---|---:|---:|
| Initial native |95.61%|90.0218/91.2286s|
| Initial Python |96.52%|205.0556/209.3251s|
| Confirmation native |93.48%|90.0243/90.4986s|
| Confirmation Python |96.21%|200.0556/206.4259s|

These are shared Linux AMD EPYC-Milan measurements with elapsed work pinned to CPU0. Every actual before-host observation and raw during-host counter is retained. Affinity and idle+iowait fractions establish neither OS isolation nor statistical significance.

## Validation and limits

This source passes 459 Rust tests across 35 groups, formatting and strict workspace/locked-benchmark Clippy. Independent saved review preserves all 4,740 original API outcomes: 4,347 passes, 391 assertion failures and two timeouts. Six C consumers pass. Strict shared/static runs preserve 802 methods and 809 rendered outcomes each, with the same two callback-fragmentation failures and raw exit 2; both supplemental semantic tests pass per linkage. Strict CPython includes the explicit upstream cleanup backport; benchmark sources are unmodified. C ASan/UBSan covers C harnesses linked to uninstrumented Rust with leak detection disabled. All 74 main and four auxiliary files are bound to the measured runtime; dependencies and their 156 reviewed source/license files are unchanged. Current-source sustained fuzzing and an installed PBS trial remain outstanding.

Saved assembly confirms the guards precede the avoided calls/copies, while nonzero accounting atomics and later default draining remain. Symbol sizes are machine-code sizes only; static observations provide no frequency, layout or causal elapsed-time claim.

Root ran all build, parser, benchmark and compatibility targets and primary readers. Agents prepared the source/controller adaptations and independently reconstructed saved raw outputs and host counters; source and reader peers reviewed the scripts. The coordinate agent authored this packet preparation; the API agent reviewed its source and a separate reviewer checks the assembled packet before publication. No historical performance, installed/PBS or sanitizer result is relabeled as this source. Warmed spans, success delivery and known-UTF8 feed remain follow-up proposals outside the measured runtime.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 5,832 indexed files (10,525,626 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 104 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 16 adverse rows. Four campaigns remain separate; no observations or medians are pooled. [report.json](report.json) retains full precision.

Archive SHA-256: `9fcaa3deba987b646d310d4e5f48378490e3fd7c76ec2b9fbbfe200f642172a2`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.

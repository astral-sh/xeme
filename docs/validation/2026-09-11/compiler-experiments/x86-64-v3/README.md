# Fixed x86-64-v3 compiler experiment

**Experimental and unselected.** Explicit x86-64-v3 reduces Oriole’s PGO time by 4.16% across these real XML fixtures. Oriole v3 still takes 32.63% more time than generic PGO Expat in the separately labeled post-hoc comparison. This study changes no defaults or parser source.

The measured source is allocator commit `e1d263a711e862e4f0f0018b15de9ac83a87a342`, with all 70 source files pinned. It does **not** include the later ContextText optimization. Full compatibility, Python, sanitizer/Miri and PBS validation were not run for these v3 artifacts.

## Results

Ratios compare elapsed time; lower is better. Each real aggregate covers 24 fixed conditions from six held-out XML files, using 4 KiB/64 KiB chunks and both namespace modes. Every condition retains all seven randomized cohorts.

| Real XML comparison | Normal | PGO |
| --- | ---: | ---: |
| Oriole v3 / Oriole generic | 0.977442 | 0.958434 |
| Expat v3 / Expat generic | 0.998010 | 1.053566 |
| Oriole generic / Expat generic | 1.562391 | 1.382037 |
| Oriole v3 / Expat v3 | 1.530070 | 1.259878 |
| Oriole v3 / Expat generic — **post-hoc** | 1.530187 | 1.326300 |

Normal Oriole v3 wins all 24 real conditions against generic Oriole; PGO wins 22. The two slower PGO conditions are Wayland with namespaces: 4 KiB is effectively flat (`1.00001055`), and 64 KiB regresses 0.73% (`1.00727373`). All other adverse comparisons remain in [report.json](report.json).

The four generated conditions remain separate. Oriole v3/generic ratios are `0.920210` normal and `0.914285` PGO; corresponding Expat ratios are `0.940410` and `0.998402`. Generated Oriole v3 still takes about `4.55×` generic Expat’s time under PGO.

The post-hoc ratio was requested after both runs finished because PGO Expat v3 regressed in 23 of 24 real conditions. It uses the original worker samples: divide worker medians within each cohort, take the median of seven paired ratios per condition, then the geometric mean across conditions. It does not multiply aggregates or add observations. PGO Oriole v3 wins 6 of 24 real conditions against generic PGO Expat. The four planned ratios remain unchanged.

## Build and validation scope

Both engines receive exactly one explicit ISA setting in normal, generate and use phases: Rust `-Ctarget-cpu=x86-64-v3`; GCC `-march=x86-64-v3`. Rust retains Ohm 1.98.1 / LLVM 22.1.8, O3, ThinLTO and one codegen unit. Expat retains GCC 13.3, O3 and no LTO. All nine Rust and 24 Expat compiler/link vectors were checked against generic controls without changing compiler metadata or other flags.

The generic controls are earlier frozen builds; v3 builds and profiles are fresh. Both use the unchanged 12-fixture generated-only G288 schedule. Normal artifacts have no PGO: their replay checks correctness. All 1,728 v3 generated records match their corresponding same-engine controls. The older Expat input manifest’s documented stale captions remain intact; actual fixture bytes, schedule and records agree with the current portable manifest.

Native measurements cover **1,792 workers and 283,136 samples**, including 281,120 timed nonwarmup samples. Worker callback hashes, element/text-byte counts, seeded ordering and arithmetic all match. These are parser/consumer-driver timings, not complete application runs or full XML API coverage. Linux capability metadata for CPUs 0, 3 and 5 is retained; a general ISA deployment path is outside this study.

## Evidence and portable replay

Run with Python 3.12.13 (the tested interpreter):

```sh
python3 -I -S -B verify.py
```

The verifier reads only this package. It checks archive hashes, 70 Oriole and 167 Expat source files, six pipeline helpers, raw compiler vector differences, and saved training/profile bindings. It replays the original native and post-hoc auditors through archive-only file access and reproduces their output bytes exactly. It does not execute a parser, compiler, profiler or benchmark.

[evidence.tar.gz](evidence.tar.gz) retains every original worker/report unchanged, preparations, source snapshots, compiler/profile/training evidence, independent audits and reviewer corrections. [members.json.gz](members.json.gz) maps all original paths to stored bytes and records exclusions. Exact duplicate files share a hash-addressed member. Oriole, Expat and real-corpus notices are included.

Compiled libraries and tool binaries are **hash identities only**. Source capsules are expanded into included source files; their original container hashes remain recorded identities. Reusable build trees and unrelated historical archives are excluded. Portable replay does not revalidate omitted binary bytes. Its scope is distinct from the original local byte audit and from new runtime compatibility testing.

All producer builds and campaigns passed first attempts. Two reviewer-only assertion mistakes are retained with their corrections; no producer rerun occurred. Package assembly and initial portable verification also passed first attempts. A subsequent verifier-only change pins both original numerical-auditor digests before execution; its replay also passed. The initial verifier/index and amendment record are retained, while archive payload bytes remain unchanged. [report.json](report.json) contains exact ratios, adverse conditions, identities and verification scope; [files.json](files.json) indexes top-level files.

# PR #240: Reuse detached callback frames across input feeds

**Evidence status: complete.** Measured source [`49352b6`](https://github.com/astral-sh/xeme/commit/49352b61dd688a16fd3252b6cba2d68225a94363) against `377e2d6fdc4e7d22cf1043277089f1df0c2e3df6` and the unchanged Expat control. All per-condition outcomes are retained, including any generated-control regressions.

## Matched timing

Ratios are candidate time / control time; lower is faster. Each epoch independently uses seven randomized matched rounds, twenty native or ten Python measured parses after one warmup, pinned to CPU 0. Aggregates are equal-weight geometric means of per-condition median paired ratios. Epochs and corpora are never pooled.

| Group | Conditions/epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| native-tuning | 24 | 0.9803× | 1.1762× | 0.9819× | 1.1712× |
| native-holdout | 20 | 0.9810× | 1.7643× | 0.9774× | 1.7711× |
| native-generated | 16 | 1.0027× | 1.4723× | 0.9971× | 1.4843× |
| python-tuning | 24 | 0.9951× | 1.0469× | 0.9923× | 1.0489× |
| python-holdout | 20 | 0.9963× | 1.2316× | 0.9943× | 1.2218× |

Generated controls have 7/16 observed slower conditions in epoch 1 and 7/16 in epoch 2. These conditions remain visible alongside the real-project results.

Tuning and previously observed holdout files are regression corpora; these results do not establish performance on unseen inputs. Xeme uses generic x86-64 O3/ThinLTO with one codegen unit; Expat uses GCC O3 without LTO. Both use the same system allocator. CPython 3.12.13 consumers use identical unmodified sources and O2 flags. Timings cover parser/consumer work, not full applications or installed distributions. Shared-host affinity does not establish isolation or statistical significance.

## Targeted conditions

| Slice | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: |
| Qt / native | 1.0030× | 3.1057× | 1.0026× | 3.1537× |
| LibreOffice / native | 1.0054× | 2.6618× | 0.9886× | 2.6311× |
| Qt / ElementTree | 1.0148× | 1.9347× | 1.0033× | 1.9116× |
| LibreOffice / ElementTree | 1.0124× | 1.8601× | 1.0051× | 1.8391× |
| Generated namespaces / namespaces on | 1.0219× | 3.1524× | 1.0158× | 3.1751× |
| Generated references | 0.9927× | 2.3707× | 0.9849× | 2.3741× |
| Generated elements | 1.0111× | 1.7059× | 1.0038× | 1.7477× |
| Generated plain text | 0.9792× | 0.5121× | 0.9784× | 0.5130× |
| Tuning / native namespaces on | 0.9815× | 1.2275× | 0.9846× | 1.2224× |
| Observed holdout / native namespaces on | 0.9795× | 1.7880× | 0.9782× | 1.7972× |

Generated “references” means predefined and numeric references in text; it does not measure general DTD entity expansion.

## Regressions

- Epoch 1, observed slower conditions: native-tuning: 5, native-holdout: 5, native-generated: 7, python-tuning: 11, python-holdout: 7.
- Epoch 2, observed slower conditions: native-tuning: 5, native-holdout: 3, native-generated: 7, python-tuning: 4, python-holdout: 7.

17 conditions are slower in both epochs. The largest repeat regressions (ranked by the smaller of their two percentage increases) follow; all rows remain in [the combined-epoch condition CSV](<2026-09-13-frame-performance.csv>).

| Group / condition | Epoch 1 | Epoch 2 |
| --- | ---: | ---: |
| native-generated / `namespaces/65536/namespaces-0` | +3.52% | +3.91% |
| native-generated / `namespaces/4096/namespaces-0` | +3.55% | +1.79% |
| native-generated / `namespaces/4096/namespaces-1` | +1.67% | +1.62% |
| native-generated / `namespaces/65536/namespaces-1` | +2.72% | +1.54% |
| native-generated / `elements/65536/namespaces-1` | +1.90% | +1.35% |
| python-holdout / `qt/65536/elementtree` | +1.37% | +0.99% |
| native-tuning / `batik/65536/namespaces-0` | +0.71% | +0.70% |
| python-tuning / `batik/65536/elementtree` | +1.75% | +0.45% |
| python-holdout / `libreoffice/4096/elementtree` | +2.59% | +0.40% |
| native-tuning / `gtk/65536/namespaces-0` | +0.92% | +0.30% |

## Correctness and allocation evidence

- API: candidate retains 509 known failures across 4740 configurations; Expat retains 0. A passing regression gate does not relabel strict upstream failures.
- W3C: 6003 rows/engine; conformance failures {'reference': 960, 'xeme': 960}. Differential regression gate: True.
- CPython: raw suite exit 2; regression gate exit 0; semantic check exit 0. The two known callback-fragmentation assertions remain separate strict failures.
- Workspace records: workspace-tests.log: 522 passes, 0 failures / 37 groups; targeted-tests.log: 2 passes, 0 failures / 2 groups.
- Full validation records: validation-summary.json: `study/frame/validation-summary.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>). No new sanitizer/Miri or installed-PBS result is inferred from these gates.

Allocation requests were measured separately through the C memory suite, including both namespace modes. They are not timing samples or RSS measurements.

| Corpus / feed | Requests candidate/base | Fewer / equal / more | Peak requested bytes Δ range |
| --- | ---: | ---: | ---: |
| tuning / 4096 | 0.6930× | 12 / 0 / 0 | +239 to +685 |
| tuning / 65536 | 0.9674× | 4 / 8 / 0 | +175 to +256 |
| holdout / 4096 | 0.5338× | 10 / 0 / 0 | +256 to +434 |
| holdout / 65536 | 0.9301× | 10 / 0 / 0 | +256 to +330 |

Full allocation records: method: `study/frame/allocations/README.md` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); every observation: `study/frame/allocations/results.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).

## Source identities and raw records

- Candidate build: receipt: `study/frame/committed-build/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `fa0c0c74d06f5fe9097e51517a1445027db9efbd5f5ed5434f749dcf66fb263f`.
- Baseline build: receipt: `study/baseline/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `8b845689276e805fe29578d6163b60ca30e5f4e9cd3968a3adb1ea668ccff584`.
- Expat build: receipt: `control/expat/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
- Matched consumers: build.json: `study/frame/consumers/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Raw campaign directories: epoch 1: `study/frame/epoch-1` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); epoch 2: `study/frame/epoch-2` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Compact results: [2026-09-13-frame-performance.json](<2026-09-13-frame-performance.json>); all conditions: [2026-09-13-frame-performance.csv](<2026-09-13-frame-performance.csv>).
- Public raw archive: [xeme-performance-2026-09-13.tar.zst](https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst); SHA-256 `fea3ce48d58394664dc19823118e86fd2ee1bdb5816dffee727deb1a31dce680`.
- Member paths above are relative to that exact archive. Absolute local paths in the JSON/CSV are labeled provenance; they are not public download links.

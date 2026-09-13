# PR #239: Reduce predefined and numeric reference dispatch overhead

**Evidence status: complete.** Measured source [`7f6ccf4`](https://github.com/astral-sh/xeme/commit/7f6ccf484b6377c69b8fd8f3f927c85428ca4013) against `377e2d6fdc4e7d22cf1043277089f1df0c2e3df6` and the unchanged Expat control. All per-condition outcomes are retained, including any generated-control regressions.

## Matched timing

Ratios are candidate time / control time; lower is faster. Each epoch independently uses seven randomized matched rounds, twenty native or ten Python measured parses after one warmup, pinned to CPU 0. Aggregates are equal-weight geometric means of per-condition median paired ratios. Epochs and corpora are never pooled.

| Group | Conditions/epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| native-tuning | 24 | 0.9986× | 1.1980× | 1.0034× | 1.1952× |
| native-holdout | 20 | 0.9976× | 1.8131× | 0.9954× | 1.8115× |
| native-generated | 16 | 0.9938× | 1.4716× | 0.9914× | 1.4801× |
| python-tuning | 24 | 1.0027× | 1.0609× | 0.9997× | 1.0547× |
| python-holdout | 20 | 1.0027× | 1.2362× | 0.9987× | 1.2326× |

Generated controls have 9/16 observed slower conditions in epoch 1 and 5/16 in epoch 2. These conditions remain visible alongside the real-project results.

Tuning and previously observed holdout files are regression corpora; these results do not establish performance on unseen inputs. Xeme uses generic x86-64 O3/ThinLTO with one codegen unit; Expat uses GCC O3 without LTO. Both use the same system allocator. CPython 3.12.13 consumers use identical unmodified sources and O2 flags. Timings cover parser/consumer work, not full applications or installed distributions. Shared-host affinity does not establish isolation or statistical significance.

## Targeted conditions

| Slice | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: |
| Qt / native | 1.0032× | 3.1525× | 0.9964× | 3.1486× |
| LibreOffice / native | 0.9962× | 2.6452× | 0.9873× | 2.6269× |
| Qt / ElementTree | 0.9944× | 1.8995× | 0.9881× | 1.8807× |
| LibreOffice / ElementTree | 0.9807× | 1.8450× | 0.9801× | 1.8385× |
| Generated namespaces / namespaces on | 1.0153× | 3.1432× | 1.0113× | 3.2129× |
| Generated references | 0.9536× | 2.2836× | 0.9505× | 2.2861× |
| Generated elements | 0.9970× | 1.7235× | 0.9959× | 1.7501× |
| Generated plain text | 1.0003× | 0.5228× | 0.9989× | 0.5219× |
| Tuning / native namespaces on | 1.0057× | 1.2496× | 1.0083× | 1.2500× |
| Observed holdout / native namespaces on | 0.9933× | 1.8313× | 0.9947× | 1.8393× |

Generated “references” means predefined and numeric references in text; it does not measure general DTD entity expansion.

## Regressions

- Epoch 1, observed slower conditions: native-tuning: 10, native-holdout: 4, native-generated: 9, python-tuning: 14, python-holdout: 12.
- Epoch 2, observed slower conditions: native-tuning: 15, native-holdout: 5, native-generated: 5, python-tuning: 9, python-holdout: 10.

29 conditions are slower in both epochs. The largest repeat regressions (ranked by the smaller of their two percentage increases) follow; all rows remain in [the combined-epoch condition CSV](<2026-09-13-reference-performance.csv>).

| Group / condition | Epoch 1 | Epoch 2 |
| --- | ---: | ---: |
| native-generated / `namespaces/4096/namespaces-0` | +3.92% | +3.40% |
| native-generated / `namespaces/65536/namespaces-0` | +3.37% | +3.07% |
| native-tuning / `maven/65536/namespaces-1` | +3.04% | +3.27% |
| python-holdout / `hadoop/65536/elementtree` | +2.51% | +2.49% |
| native-tuning / `maven/4096/namespaces-1` | +2.08% | +2.17% |
| native-generated / `namespaces/4096/namespaces-1` | +1.57% | +1.53% |
| python-holdout / `hadoop/4096/elementtree` | +2.29% | +1.30% |
| native-tuning / `gtk/65536/namespaces-1` | +1.52% | +1.25% |
| python-tuning / `gtk/65536/pyexpat-events` | +1.03% | +0.82% |
| python-tuning / `gtk/4096/elementtree` | +0.77% | +2.32% |

## Correctness and allocation evidence

- API: candidate retains 509 known failures across 4740 configurations; Expat retains 0. A passing regression gate does not relabel strict upstream failures.
- W3C: 6003 rows/engine; conformance failures {'reference': 960, 'xeme': 960}. Differential regression gate: True.
- CPython: raw suite exit 2; regression gate exit 0; semantic check exit 0. The two known callback-fragmentation assertions remain separate strict failures.
- Workspace records: xeme-perf-reference-test.log: 522 passes, 0 failures / 37 groups.
- Full validation records: validation-summary.json: `study/reference/validation-summary.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>). No new sanitizer/Miri or installed-PBS result is inferred from these gates.

Allocation requests were measured separately through the C memory suite, including both namespace modes. They are not timing samples or RSS measurements.

| Corpus / feed | Requests candidate/base | Fewer / equal / more | Peak requested bytes Δ range |
| --- | ---: | ---: | ---: |
| tuning / 4096 | 1.0000× | 0 / 12 / 0 | +0 to +0 |
| tuning / 65536 | 1.0000× | 0 / 12 / 0 | +0 to +0 |
| holdout / 4096 | 1.0000× | 0 / 10 / 0 | +0 to +0 |
| holdout / 65536 | 1.0000× | 0 / 10 / 0 | +0 to +0 |

Full allocation records: method: `study/frame/allocations/README.md` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); every observation: `study/frame/allocations/results.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).

## Source identities and raw records

- Candidate build: receipt: `study/reference/build/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `626ad24736c9fc313c2eefaf4d728b6bfd444af905080746dddb7877f8cd00de`.
- Baseline build: receipt: `study/baseline/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `8b845689276e805fe29578d6163b60ca30e5f4e9cd3968a3adb1ea668ccff584`.
- Expat build: receipt: `control/expat/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
- Matched consumers: build.json: `study/reference/consumers/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Raw campaign directories: epoch 1: `study/reference/epoch-1` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); epoch 2: `study/reference/epoch-2` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Compact results: [2026-09-13-reference-performance.json](<2026-09-13-reference-performance.json>); all conditions: [2026-09-13-reference-performance.csv](<2026-09-13-reference-performance.csv>).
- Public raw archive: [xeme-performance-2026-09-13.tar.zst](https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst); SHA-256 `fea3ce48d58394664dc19823118e86fd2ee1bdb5816dffee727deb1a31dce680`.
- Member paths above are relative to that exact archive. Absolute local paths in the JSON/CSV are labeled provenance; they are not public download links.

# PR #237: Keep tag fast paths after DTDs without attribute declarations

**Evidence status: complete.** Measured source [`86868a7`](https://github.com/astral-sh/xeme/commit/86868a706465dc389d8b6bb536da7bff6c01c66b) against `377e2d6fdc4e7d22cf1043277089f1df0c2e3df6` and the unchanged Expat control. All per-condition outcomes are retained, including any generated-control regressions.

## Matched timing

Ratios are candidate time / control time; lower is faster. Each epoch independently uses seven randomized matched rounds, twenty native or ten Python measured parses after one warmup, pinned to CPU 0. Aggregates are equal-weight geometric means of per-condition median paired ratios. Epochs and corpora are never pooled.

| Group | Conditions/epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| native-tuning | 24 | 0.9651× | 1.1507× | 0.9702× | 1.1558× |
| native-holdout | 20 | 0.7901× | 1.4286× | 0.7915× | 1.4304× |
| native-generated | 16 | 1.0197× | 1.5099× | 1.0188× | 1.5130× |
| python-tuning | 24 | 0.9925× | 1.0435× | 0.9879× | 1.0405× |
| python-holdout | 20 | 0.9051× | 1.1841× | 0.8805× | 1.0880× |

Generated controls have 15/16 observed slower conditions in epoch 1 and 16/16 in epoch 2. These conditions remain visible alongside the real-project results.

Tuning and previously observed holdout files are regression corpora; these results do not establish performance on unseen inputs. Xeme uses generic x86-64 O3/ThinLTO with one codegen unit; Expat uses GCC O3 without LTO. Both use the same system allocator. CPython 3.12.13 consumers use identical unmodified sources and O2 flags. Timings cover parser/consumer work, not full applications or installed distributions. Shared-host affinity does not establish isolation or statistical significance.

## Targeted conditions

| Slice | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: |
| Qt / native | 0.5140× | 1.6107× | 0.5143× | 1.6203× |
| LibreOffice / native | 0.6271× | 1.6623× | 0.6269× | 1.6586× |
| Qt / ElementTree | 0.6350× | 1.0878× | 0.6233× | 1.1881× |
| LibreOffice / ElementTree | 0.8792× | 3.2464× | 0.7535× | 1.4025× |
| Generated namespaces / namespaces on | 1.0087× | 3.1761× | 1.0145× | 3.1652× |
| Generated references | 1.0323× | 2.4898× | 1.0275× | 2.4625× |
| Generated elements | 1.0315× | 1.7493× | 1.0249× | 1.7841× |
| Generated plain text | 1.0062× | 0.5261× | 1.0101× | 0.5273× |
| Tuning / native namespaces on | 0.9701× | 1.2027× | 0.9752× | 1.2084× |
| Observed holdout / native namespaces on | 0.8036× | 1.4758× | 0.8040× | 1.4701× |

LibreOffice ElementTree varies materially between epochs: candidate/Expat is 3.2464× in epoch 1 and 1.4025× in epoch 2. All observations are retained. The Python holdout aggregate includes this variation; neither epoch establishes a stable LibreOffice ElementTree ratio against Expat.

Generated “references” means predefined and numeric references in text; it does not measure general DTD entity expansion.

## Regressions

- Epoch 1, observed slower conditions: native-tuning: 8, native-holdout: 2, native-generated: 15, python-tuning: 13, python-holdout: 3.
- Epoch 2, observed slower conditions: native-tuning: 9, native-holdout: 3, native-generated: 16, python-tuning: 9, python-holdout: 2.

31 conditions are slower in both epochs. The largest repeat regressions (ranked by the smaller of their two percentage increases) follow; all rows remain in [the combined-epoch condition CSV](<2026-09-13-doctype-performance.csv>).

| Group / condition | Epoch 1 | Epoch 2 |
| --- | ---: | ---: |
| native-generated / `elements/65536/namespaces-0` | +4.27% | +3.64% |
| native-generated / `entities/65536/namespaces-1` | +2.93% | +3.33% |
| native-generated / `entities/65536/namespaces-0` | +3.03% | +2.75% |
| python-tuning / `gtk/65536/elementtree` | +2.52% | +2.75% |
| native-generated / `elements/65536/namespaces-1` | +2.34% | +2.56% |
| native-generated / `elements/4096/namespaces-0` | +3.86% | +2.33% |
| native-generated / `entities/4096/namespaces-0` | +4.90% | +2.29% |
| native-generated / `entities/4096/namespaces-1` | +2.09% | +2.64% |
| python-tuning / `vulkan/4096/pyexpat-events` | +2.28% | +1.93% |
| python-tuning / `vulkan/4096/elementtree` | +2.27% | +1.72% |

## Correctness and allocation evidence

- API: candidate retains 507 known failures across 4740 configurations; Expat retains 0. A passing regression gate does not relabel strict upstream failures.
- W3C: 6003 rows/engine; conformance failures {'reference': 960, 'xeme': 960}. Differential regression gate: True.
- CPython: raw suite exit 2; regression gate exit 0; semantic check exit 0. The two known callback-fragmentation assertions remain separate strict failures.
- Workspace records: doctype-adapter-tests.log: 0 passes, 0 failures / 0 groups; doctype-workspace-tests.log: 522 passes, 0 failures / 37 groups.
- Full validation records: validation-summary.json: `study/doctype/validation-summary.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>). No new sanitizer/Miri or installed-PBS result is inferred from these gates.

Allocation requests were measured separately through the C memory suite, including both namespace modes. They are not timing samples or RSS measurements.

| Corpus / feed | Requests candidate/base | Fewer / equal / more | Peak requested bytes Δ range |
| --- | ---: | ---: | ---: |
| tuning / 4096 | 0.9494× | 2 / 10 / 0 | +0 to +1652 |
| tuning / 65536 | 0.9335× | 2 / 10 / 0 | +0 to +1684 |
| holdout / 4096 | 0.6459× | 4 / 6 / 0 | -85 to +510 |
| holdout / 65536 | 0.5672× | 4 / 6 / 0 | -89 to +488 |

Full allocation records: method: `study/frame/allocations/README.md` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); every observation: `study/frame/allocations/results.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).

## Source identities and raw records

- Candidate build: receipt: `study/doctype/build/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `b6087607479279233401b85e8a14e1c53cca6d989edc72622d311389898fbcf6`.
- Baseline build: receipt: `study/baseline/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `8b845689276e805fe29578d6163b60ca30e5f4e9cd3968a3adb1ea668ccff584`.
- Expat build: receipt: `control/expat/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
- Matched consumers: build.json: `study/doctype/consumers/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Raw campaign directories: epoch 1: `study/doctype/epoch-1` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); epoch 2: `study/doctype/epoch-2` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Compact results: [2026-09-13-doctype-performance.json](<2026-09-13-doctype-performance.json>); all conditions: [2026-09-13-doctype-performance.csv](<2026-09-13-doctype-performance.csv>).
- Public raw archive: [xeme-performance-2026-09-13.tar.zst](https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst); SHA-256 `fea3ce48d58394664dc19823118e86fd2ee1bdb5816dffee727deb1a31dce680`.
- Member paths above are relative to that exact archive. Absolute local paths in the JSON/CSV are labeled provenance; they are not public download links.

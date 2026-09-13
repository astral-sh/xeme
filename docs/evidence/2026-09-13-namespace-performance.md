# PR #238: Expand small namespace attribute lists once and use detached frames

**Evidence status: complete.** Measured source [`9ab8f47`](https://github.com/astral-sh/xeme/commit/9ab8f47189308814e5f685496bc450048e89372d) against `86868a706465dc389d8b6bb536da7bff6c01c66b` and the unchanged Expat control. All per-condition outcomes are retained, including any generated-control regressions.

This PR is stacked on #237. Candidate/base ratios isolate the namespace change; candidate/Expat ratios include its DOCTYPE parent. Gains from separate PRs are not additive.

## Matched timing

Ratios are candidate time / control time; lower is faster. Each epoch independently uses seven randomized matched rounds, twenty native or ten Python measured parses after one warmup, pinned to CPU 0. Aggregates are equal-weight geometric means of per-condition median paired ratios. Epochs and corpora are never pooled.

| Group | Conditions/epoch | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| native-tuning | 24 | 0.9846× | 1.1346× | 0.9886× | 1.1481× |
| native-holdout | 20 | 0.9702× | 1.3670× | 0.9657× | 1.3861× |
| native-generated | 16 | 0.9417× | 1.4174× | 0.9387× | 1.4169× |
| python-tuning | 24 | 0.9917× | 1.0325× | 0.9990× | 1.0421× |
| python-holdout | 20 | 0.9713× | 1.0578× | 0.9706× | 1.0564× |

Generated controls have 0/16 observed slower conditions in epoch 1 and 0/16 in epoch 2. These conditions remain visible alongside the real-project results.

Tuning and previously observed holdout files are regression corpora; these results do not establish performance on unseen inputs. Xeme uses generic x86-64 O3/ThinLTO with one codegen unit; Expat uses GCC O3 without LTO. Both use the same system allocator. CPython 3.12.13 consumers use identical unmodified sources and O2 flags. Timings cover parser/consumer work, not full applications or installed distributions. Shared-host affinity does not establish isolation or statistical significance.

## Targeted conditions

| Slice | Epoch 1 / base | Epoch 1 / Expat | Epoch 2 / base | Epoch 2 / Expat |
| --- | ---: | ---: | ---: | ---: |
| Qt / native | 0.9774× | 1.5307× | 0.9734× | 1.5805× |
| LibreOffice / native | 0.9159× | 1.5139× | 0.9073× | 1.5023× |
| Qt / ElementTree | 0.9637× | 1.1654× | 0.9744× | 1.1728× |
| LibreOffice / ElementTree | 0.8912× | 1.2611× | 0.8973× | 1.2609× |
| Generated namespaces / namespaces on | 0.6992× | 2.2279× | 0.6989× | 2.1780× |
| Generated references | 0.9892× | 2.4372× | 0.9879× | 2.4297× |
| Generated elements | 0.9839× | 1.7210× | 0.9806× | 1.7538× |
| Generated plain text | 0.9777× | 0.5131× | 0.9764× | 0.5136× |
| Tuning / native namespaces on | 0.9802× | 1.1772× | 0.9895× | 1.2011× |
| Observed holdout / native namespaces on | 0.9568× | 1.3799× | 0.9481× | 1.4011× |

Generated “references” means predefined and numeric references in text; it does not measure general DTD entity expansion.

## Regressions

- Epoch 1, observed slower conditions: native-tuning: 6, native-holdout: 3, native-generated: 0, python-tuning: 8, python-holdout: 3.
- Epoch 2, observed slower conditions: native-tuning: 3, native-holdout: 0, native-generated: 0, python-tuning: 11, python-holdout: 1.

8 conditions are slower in both epochs. The largest repeat regressions (ranked by the smaller of their two percentage increases) follow; all rows remain in [the combined-epoch condition CSV](<2026-09-13-namespace-performance.csv>).

| Group / condition | Epoch 1 | Epoch 2 |
| --- | ---: | ---: |
| python-tuning / `docbook/65536/elementtree` | +2.22% | +3.98% |
| python-tuning / `maven/4096/elementtree` | +1.81% | +2.58% |
| python-tuning / `maven/65536/elementtree` | +3.29% | +1.59% |
| native-tuning / `maven/65536/namespaces-1` | +1.24% | +1.60% |
| python-tuning / `docbook/4096/pyexpat-events` | +0.78% | +0.52% |
| native-tuning / `maven/4096/namespaces-1` | +0.22% | +1.18% |
| python-tuning / `maven/65536/pyexpat-events` | +0.70% | +0.15% |
| python-tuning / `gtk/65536/pyexpat-events` | +0.14% | +0.28% |

## Correctness and allocation evidence

- API: candidate retains 495 known failures across 4740 configurations; Expat retains 0. A passing regression gate does not relabel strict upstream failures.
- W3C: 6003 rows/engine; conformance failures {'reference': 960, 'xeme': 960}. Differential regression gate: True.
- CPython: raw suite exit 2; regression gate exit 0; semantic check exit 0. The two known callback-fragmentation assertions remain separate strict failures.
- Workspace records: stacked-workspace-tests.log: 524 passes, 0 failures / 37 groups.
- Full validation records: validation-summary.json: `study/namespace/validation-summary.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>). No new sanitizer/Miri or installed-PBS result is inferred from these gates.

Allocation requests were measured separately through the C memory suite, including both namespace modes. They are not timing samples or RSS measurements.

| Corpus / feed | Requests candidate/base | Fewer / equal / more | Peak requested bytes Δ range |
| --- | ---: | ---: | ---: |
| tuning / 4096 | 0.9837× | 3 / 9 / 0 | -23 to +0 |
| tuning / 65536 | 0.9788× | 3 / 9 / 0 | -23 to +0 |
| holdout / 4096 | 0.9434× | 1 / 9 / 0 | -277 to +0 |
| holdout / 65536 | 0.9392× | 1 / 9 / 0 | -333 to +0 |

Full allocation records: method: `study/frame/allocations/README.md` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); every observation: `study/frame/allocations/results.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).

## Source identities and raw records

- Candidate build: receipt: `study/namespace/build/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `ad995d7e031a9686165b3377b6d3ac53ee6b134975839b11c1673ac0be60a48a`.
- Baseline build: receipt: `study/doctype/build/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `b6087607479279233401b85e8a14e1c53cca6d989edc72622d311389898fbcf6`.
- Expat build: receipt: `control/expat/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); library SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.
- Matched consumers: build.json: `study/namespace/consumers/build.json` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Raw campaign directories: epoch 1: `study/namespace/epoch-1` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>); epoch 2: `study/namespace/epoch-2` in the [raw archive](<https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst>).
- Compact results: [2026-09-13-namespace-performance.json](<2026-09-13-namespace-performance.json>); all conditions: [2026-09-13-namespace-performance.csv](<2026-09-13-namespace-performance.csv>).
- Public raw archive: [xeme-performance-2026-09-13.tar.zst](https://github.com/astral-sh/xeme/releases/download/performance-2026-09-13/xeme-performance-2026-09-13.tar.zst); SHA-256 `fea3ce48d58394664dc19823118e86fd2ee1bdb5816dffee727deb1a31dce680`.
- Member paths above are relative to that exact archive. Absolute local paths in the JSON/CSV are labeled provenance; they are not public download links.

# QName validation confirmation

The second measurement epoch repeats a small performance improvement from
[the lean QName validation change, #176](https://github.com/astral-sh/oriole/pull/176).
We recommend merging this change: it reuses validation already performed by the
native scanner, with no additional cache state or allocation. This report adds
performance confirmation; it does not establish a new installed CPython or
python-build-standalone qualification.

The measured source is `735948097c48a2a30cdc88be49632c1410b4edc8`. Both epochs use
the same frozen candidate and qualified `5d983f7e` libraries. The earlier
[experiment report](../namespace-name-experiments/README.md), published in
[#177](https://github.com/astral-sh/oriole/pull/177), retains the first epoch and
the rejected cache alternatives unchanged.

## Results

Each ratio is an equal-weight geometric mean over 24 real-project conditions.
Lower is better; `1.20` is the agreed aggregate limit relative to Expat.
The epochs are separate measurements, not pooled samples.

| Epoch | Native / qualified | Native / Expat | CPython / qualified | CPython / Expat |
| --- | ---: | ---: | ---: | ---: |
| First | 0.99151 | 1.17205 | 0.99427 | 1.05000 |
| Confirmation | 0.99280 | 1.17392 | 0.99273 | 1.05038 |

In the confirmation, native parsing takes **0.72% less time** than the qualified
runtime and CPython takes **0.73% less time**. The gaps to Expat are **17.39%** and
**5.04%**, respectively. Both epochs improve both real-project aggregates.

All adverse conditions remain in the reports. The confirmation improves 14 of
24 native conditions and 13 of 24 CPython conditions. Ten native and eleven
CPython conditions regress; five native and three CPython conditions exceed 1%,
and none exceed 2%. The largest native regression is Batik, 4 KiB chunks,
namespaces disabled, at 1.89%. The largest CPython regression is Vulkan through
pyexpat, 64 KiB chunks, at 1.87%.

The four generated native conditions are excluded from the real-project target.
Their aggregate remains **3.32× Expat** in the confirmation, versus 3.35× in the
first epoch. The result does not put every workload within 20% of Expat.

Per-condition data: [native CSV](native/conditions.csv) and
[CPython CSV](python/normal-conditions.csv). Full comparisons for both epochs are
in [summary.json](summary.json).

## Method and verification

We reused the exact normal libraries and all six previously compiled, unmodified
CPython 3.12.13 extensions. No rebuilding, PGO, allocator change, or consumer
patch was involved. The existing build uses generic x86-64, Rust O3, ThinLTO,
and one codegen unit; the Expat 2.8.4 control uses normal GCC O3. The native
driver, XML corpus, seven paired rounds, seeds, iteration counts, and timing
boundaries remain unchanged.

All six pinned XML projects run at 4 KiB and 64 KiB chunk sizes. Native conditions
cover namespaces enabled and disabled; CPython conditions cover ElementTree and
pyexpat. Each condition uses the median of seven paired process-median ratios.
Input reading, imports, and canonical output checks are outside timing; parser
lifecycle and callback work are included. CPython result destruction is included.

Fresh canonical preflights precede timing. The saved-output readers reconstruct
all **1,248 workers and 132,540 samples**: 672 native workers / 106,176 samples and
576 CPython workers / 26,364 samples. They verify input and library hashes,
extension and parser origins, outputs, worker order, process medians, ratios,
and aggregates. Both readers pass. The controllers run sequentially; all target
processes and host monitors exited and were reaped. The controller elapsed time
was approximately 334 seconds.

The reports are [native readback](native/review.json),
[CPython preflight readback](python/preflight-review.json), and
[CPython timing readback](python/normal-review.json). The original build and its
source binding are retained under [source/](source/source-commit.json).

## Scope and remaining gaps

This was a shared host with CPU affinity, not OS-enforced isolation. Raw pairs
include outliers. Two agreeing epochs support a repeated small gain; they do not
establish statistical significance or guarantee an application-level speedup.

No upstream API, strict CPython, sanitizer, W3C, or installed distribution tests
were rerun for this confirmation. The earlier exact-source qualification remains
the evidence for 484 Rust tests, sanitizer replay and mutation, and unchanged
upstream outcomes: **391 of 4,740 API configurations fail**, and the same **two
strict CPython callback-grouping failures** remain. Benchmark canonical checks
coalesce adjacent text callbacks and do not replace those strict assertions.
The earlier sanitizer campaign disabled leak detection. No new installed
CPython/PBS qualification is claimed.

## Evidence

[COPY_INDEX.json](COPY_INDEX.json) records the 45 exact copied files, their local
sources, sizes, and SHA-256 hashes. [LOCAL_RAW.md](LOCAL_RAW.md) identifies the
uncopied raw data and explains the local-path dependencies of the controllers.
The three generated documentation/index files are listed separately from copied
evidence. Existing evidence was not rewritten.

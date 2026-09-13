# Capacity-fixed runtime: CPython measurements

The selected capacity fix leaves CPython performance effectively unchanged. PGO
takes **1.1465× Expat's time** across the 24 conditions, so the target of roughly
1.10× remains unmet. The pyexpat-event aggregate is within that target; ElementTree
and many individual conditions remain slower.

## Results

| Parser build | Selected / Context parent | Selected / Expat | Slower than parent | Within 1.10× Expat |
| --- | ---: | ---: | ---: | ---: |
| Normal | 1.000060 | 1.275813 | 14/24 | 4/24 |
| Generated-corpus PGO | 1.000163 | 1.146520 | 14/24 | 6/24 |

| CPython consumer | Normal / Expat | PGO / Expat |
| --- | ---: | ---: |
| ElementTree | 1.359323 | 1.197582 |
| pyexpat events | 1.197434 | 1.097634 |

Lower is faster. [conditions.csv](conditions.csv) retains all 48 comparisons,
including every adverse result. These are fixed campaigns on a shared Linux AMD
EPYC-Milan host. The small parent differences do not establish a speed change.
The separate [native capacity study](../arena-capacity-bound/) measured 1.3673×
Expat with PGO; this report adds no new native timing.

Each build measures six original project XML files, 4 KiB and 64 KiB feeds, and
two actual CPython consumers. Seven seeded, interleaved pairs per condition use
seed `202609104412`. Each condition reports the median of paired time ratios;
the table uses their geometric mean. No conditions or samples were discarded
after observing results. Each worker discards its predetermined first warmup.

Timing includes parser creation, feeding, finalization, callbacks and explicit
destruction. Input reads, imports, canonical validation and explicit garbage
collection are outside timing; automatic garbage collection remains enabled.
Canonical checks coalesce adjacent text callbacks. These runs parse project XML
fixtures, rather than execute whole applications or test exact text fragmentation.

## Source and consumers

The selected runtime is `de859c89577b2982d59b6923d682032f6a7c7800`, compared with
Context `0f28139f83b04290e62301c38d8ed489f183a88d`. Both source archives contain
72 files. The only Rust runtime change is the arena capacity bound; the inventory
also contains the inherited nested-entity C fixture change. Normal selected DSO
is `c19f1439`; PGO DSO is `4d97fe5b`.

Both Oriole builds use O3, ThinLTO and one codegen unit. PGO profiles come from
the original generated corpus, independently for each runtime; measured project
XML stays held out. Expat 2.8.4 uses its matching normal or generated-corpus PGO
library. Existing build metadata and source archives are included; full original
build evidence remains in the linked source-specific reports.

Fresh extensions compile unmodified CPython 3.12.13 `pyexpat.c` and
`_elementtree.c` at revision `3bb231a6a5dc02b95658877318bf61501a7209e9`, with
identical O2 flags. They use no allocator override or strict-suite cleanup
backport. Independent review checked all 12 compiler commands and 144 preflight
origins. All **1,152 workers and 52,728 samples** were reconstructed from raw
records, including output hashes, library/module identities, seeded order,
medians and ratios. Preparation, builds, preflights and elapsed audits passed
their first attempts. All launched processes were reaped.

This is a performance checkpoint. It does not add a full upstream API matrix,
strict CPython suite, sanitizer campaign or PBS distribution build. The
[compatibility guide](../../../compatibility.md) retains those separate gates.

## Portable evidence

The archive retains full original report JSON, every raw compressed worker
stream and specification, controllers, original reviews, source snapshots and
build metadata. Its 3,552 files omit 18 compiled binaries with explicit recorded
identities. Actual binary bytes were checked by the original preflight review.
The packaging review's initial provenance recommendations and their resolution
are both retained.

From a copied package directory, with Python 3.12:

```console
python3.12 -I -S verify.py --package . --output readback-new.json
```

The verifier checks archive hashes, both source inventories and their benchmark
bindings, then replays all raw arithmetic from relocated records. It imports no
parser or compiled binary and does not read the original absolute paths. Original
paths within reports remain identity labels. [index.json](index.json) lists every
archived file and excluded binary; [files.json](files.json) seals the package.

# Validation evidence

Current behavior is documented in the [compatibility guide](../compatibility.md).
Evidence below identifies the source it tested. Historical results do not qualify
later code, and passing a regression gate does not erase strict upstream failures.

## API baseline after rebase

The [rebase comparison](2026-09-13-rebase.md) records identical full API outcomes
on main and the review runtime, the update to 509 known allocation/API failures,
and separate raised-retry diagnostics. Original failures remain visible.

## Pre-rebase evaluation (2026-09-13)

The [2026-09-13 review report](2026-09-13-review.md) records pre-rebase runtime `ec4d068` compatibility
and bounded fuzz results, and publishes the first independent project
measurements. The holdout reports 1.7867× Expat's native time
and 1.2158× its CPython time; both miss the 1.20× goal. Tuning results remain
separate. The report identifies exact sources, all adverse conditions and durable
raw evidence.

## Historical archives

The [evidence release](https://github.com/astral-sh/oriole/releases/tag/evidence-2026-09-13)
preserves every file removed from `docs/validation` and `benchmarks/results`
through `fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f`, plus the full raw QName
confirmation directory previously retained only locally.

| Archive | Original files | Compressed bytes |
| --- | ---: | ---: |
| [Validation](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/validation-through-fe31da9.tar.zst) | 2,769 | 741,778,967 |
| [Benchmarks](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/benchmarks-through-fe31da9.tar.zst) | 205 | 173,776,474 |
| [QName raw workers](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/qname-confirmation-raw.tar.zst) | 2,455 | 2,766,931 |

The [manifest](archive-manifest.json) records archive SHA-256 hashes, original
byte counts, paths and source identities. Each archive contains an internal
`ARCHIVE-MANIFEST.json` with every member's hash. Independent verification checked
all 5,429 files, exact archive membership and the original Git blobs of all 2,974
previously tracked files. Uploaded assets match the server's SHA-256 digests.

Download an archive, compare its SHA-256 with the manifest, then extract into a
fresh directory with `tar --zstd -xf ARCHIVE`. Use each internal manifest to verify
members. Each archive has its own manifest, so extract them into separate directories.
This removes about 1.06 GB from the current tree. Git history is unchanged; a full
historical clone can still contain those bytes. Some older reports reference
additional local-only artifacts; those references do not imply that every earlier
worker is in this release.

### Useful checkpoints

- [Qualified generic Linux trial](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-12/native-start-end-namespace-declaration): source `5d983f7e`, normal library `c3e65339`; includes API, CPython, W3C, bounded ASan/fuzz and installed PBS evidence.
- [Latest merged QName confirmation](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-13/qname-confirmation): separate tuning-set epochs; full native/Python raw workers are also in the QName archive above.
- [API failure census](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-11/api-failure-census): original assertion boundaries and separate diagnostic reasoning.
- [Earlier experiment ledger](https://github.com/astral-sh/oriole/blob/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/review.md): the historical narrative, including negative and superseded experiments.

### Fuzz-count correction

Two namespace campaign totals labeled as mutation executions included 45,181
initialization executions each. Their totals of 366,981 and 367,176 correspond to
**321,800 and 321,995 mutation executions**. The archive retains original bytes;
the manifest's correction records distinguish totals, initialization and mutation.
Other historical counts retain their own recorded definitions and scope.

## New evidence

Keep concise summaries, source/artifact/input hashes, reduced regressions and
stable artifact links in this directory. Put large raw workers, source snapshots
and binaries in checksummed release assets. CI artifacts are useful during review
but expire; preserve material results before expiration.

Record initialization, replay and mutation counts separately. Benchmark reports
must retain all conditions and failures, identify the tuning or holdout corpus,
and state whether CPython used local extension modules or an installed distribution.

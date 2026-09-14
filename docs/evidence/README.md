# Test and benchmark reports

Each report records its tested revision. See the
[compatibility guide](../compatibility.md) for current behavior and known failures.

## CPython line-break callbacks

The [CPython grouping check](2026-09-13-cpython-grouping.md) records strict passes
of all six pinned XML suites with shared and static parser libraries after fixing
the two line-break callback assertions. It identifies the tested source and local
libraries; installed distributions need separate tests. The
[performance follow-up](2026-09-13-cpython-grouping-performance.md) measures the
remaining overhead compared with Xeme before the fix.

## API baseline after rebase

The [rebase comparison](2026-09-13-rebase.md) records identical full API outcomes
on main and the review runtime, the update to 509 known allocation/API failures,
and diagnostics with higher retry limits.

The [allocation-behavior check](2026-09-13-allocation-behavior.md) runs the full
public allocation suites with allocation-count assumptions relaxed, retaining
semantic assertions and adding ownership checks. Both engines passed all 1,008
reported configurations. This separate gate leaves the original 509 API failures
intact and documents the limits of its fault-injection coverage.

## Pre-rebase evaluation (2026-09-13)

The [2026-09-13 review report](2026-09-13-review.md) covers compatibility, fuzzing
and benchmarks for pre-rebase runtime `ec4d068`. On five previously unused
projects, Xeme took 1.7867× Expat's native time and 1.2158× its CPython time;
both miss the 1.20× goal. The report includes results for each condition and
links to raw data.

## Historical archives

The [archive release](https://github.com/astral-sh/oriole/releases/tag/evidence-2026-09-13)
preserves every file removed from `docs/validation` and `benchmarks/results`
through `fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f`, plus the full raw QName
confirmation directory previously retained only locally.

| Archive | Original files | Compressed bytes |
| --- | ---: | ---: |
| [Validation](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/validation-through-fe31da9.tar.zst) | 2,769 | 741,778,967 |
| [Benchmarks](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/benchmarks-through-fe31da9.tar.zst) | 205 | 173,776,474 |
| [QName raw workers](https://github.com/astral-sh/oriole/releases/download/evidence-2026-09-13/qname-confirmation-raw.tar.zst) | 2,455 | 2,766,931 |

The [manifest](archive-manifest.json) records archive SHA-256 hashes, original
byte counts, paths and source revisions. Each archive contains an internal
`ARCHIVE-MANIFEST.json` with every member's hash. Independent verification checked
all 5,429 files, exact archive membership and the original Git blobs of all 2,974
previously tracked files. Uploaded assets match the server's SHA-256 digests.

Download an archive, compare its SHA-256 with the manifest, then extract it with
`tar --zstd -xf ARCHIVE`. Use a separate directory for each archive and check its
files against the internal manifest. Some older reports link local artifacts that
are absent from this release.

### Useful checkpoints

- [Linux CPython trial](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-12/native-start-end-namespace-declaration): source `5d983f7e`, normal library `c3e65339`; includes API, CPython, W3C, ASan/fuzz and installed PBS results.
- [Latest merged QName confirmation](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-13/qname-confirmation): separate tuning-set epochs; full native/Python raw workers are also in the QName archive above.
- [API failure census](https://github.com/astral-sh/oriole/tree/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/validation/2026-09-11/api-failure-census): original assertion boundaries and separate diagnostic reasoning.
- [Earlier experiments](https://github.com/astral-sh/oriole/blob/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/docs/review.md): includes negative and superseded results.

### Fuzz-count correction

Two namespace campaign totals labeled as mutation executions included 45,181
initialization executions each. Their totals of 366,981 and 367,176 correspond to
**321,800 and 321,995 mutation executions**. The archive retains original bytes;
the manifest's correction records distinguish totals, initialization and mutation.
Other historical counts retain their own recorded definitions and scope.

## Adding reports

Keep summaries, source and input hashes, reduced regressions and artifact links
in this directory. Put large raw results, source snapshots and binaries in release
assets with checksums. Copy results needed for the report before CI artifacts expire.

Record initialization, replay and mutation counts separately. Benchmark reports
must retain all conditions and failures, identify the tuning or holdout corpus,
and state whether CPython used local extension modules or an installed distribution.

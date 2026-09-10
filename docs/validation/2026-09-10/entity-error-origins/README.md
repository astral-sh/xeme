# Entity error origins

An unbalanced internal entity now reports its reference position after the parser
pops the exhausted source. Previously, the error used the parent's position after
the reference. The error kind remains `AsynchronousEntity`; the exhausted source
has a zero byte count. This changes diagnostics without allocating or changing
which documents are accepted.

| Original upstream API matrix | Passing | Failing |
| --- | ---: | ---: |
| Published `4b11ace` runtime | 4,323 | 417 |
| This fix | 4,335 | 405 |

All 12 configurations of `test_misc_async_entity_rejected` now pass. No other
outcome changes: the same 395 test names, six chunk widths, two deferral settings,
assertions and process bounds remain in use. The full runner still exits 1 for
the remaining failures. [api-comparison.json](api-comparison.json) retains every
changed row; the [earlier classification](../version-consistent-runtime/) remains
applicable after removing its 12 error-column failures.

## Validation

The workspace passes 369 tests across all targets, strict Clippy and formatting.
The focused regression checks both an open element and an open element followed
by an empty nested entity, every chunk width, UTF-8 and both UTF-16 byte orders,
with a non-ASCII prefix and CRLF. The original five-case C probe preserves all
parse statuses and error kinds while fixing the incorrect coordinates.

Independent review adds 162 nested-source probe records across three libraries,
three encodings and three chunk sizes. Of 54 engine comparisons, 27 become fully
equal to Expat and none acquire a new difference. Eighteen retain pre-existing
nested-CDATA error-code or premature-close byte-count differences. This is not a
claim of complete diagnostic parity.

A further 3,318 strict differential observations match the preceding Oriole
runtime exactly, including callback streams and final positions. That control
is Oriole `4b11ace`, not Expat. The [summary](summary.json) records completed
controllers; the [independent review](independent-review.json) separates saved
evidence reconstruction from its additional executed probes.

## Source and evidence

The isolated runtime is based on `4b11ace`; it is integrated after `450b332`, whose
intervening changes affect tooling and evidence. All 65 recorded source files
match between the tested worktree and integration checkout. Shared/static
library hashes and the exact patch are in [source.json](source.json).

[evidence.tar.gz](evidence.tar.gz) retains source, original and candidate API
logs/results, differential traces, focused probes, Rust logs and review scripts.
Its SHA-256 is
`edebcc4ef9422ebe698d9da704bdf70d738d25c4c1e357fe76b7c246d64d14ce`.
[files.json](files.json) records all 175 members; every member was read back and
rehashed. Libraries and executables are excluded and identified by hash.

No new performance or sustained-fuzz result is claimed for this change. The
earlier benchmark, PBS and sanitizer reports retain their own runtime identities.

# Local raw benchmark evidence

This package contains exact copies of completed build records, logs, benchmark summaries, independent reviews, and small executable controllers/readers. `COPY_INDEX.json` records the original source path, relative destination, size and SHA-256 for every copied file. Source and destination hashes were checked during copying. The index itself and this explanatory document are generated staging metadata.

No raw worker files, parser binaries, Python extension binaries, or XML corpora are included. Their paths and hashes remain in the copied bindings, build records and complete reader summaries. These local paths are evidence locations on the development machine, not public download links.

| Candidate | Normal build and frozen binaries | Native raw workers | Python raw workers and candidate extensions |
|---|---|---|---|
| storage | `/tmp/oriole-namespace-name-storage-study` (binaries in `normal/`) | `/tmp/oriole-namespace-name-storage-benchmark/native/native-screen` | Not measured for this candidate |
| proof | `/tmp/oriole-namespace-name-proof-study` (binaries in `normal/`) | `/tmp/oriole-namespace-name-proof-benchmark/native/native-screen` | `/tmp/oriole-namespace-name-proof-benchmark/python/screen`; extensions in `python/consumers` |
| revision | `/tmp/oriole-namespace-binding-revision-study` (binaries in `normal/`) | `/tmp/oriole-namespace-binding-revision-benchmark/native/native-screen` | `/tmp/oriole-namespace-binding-revision-benchmark/python/screen`; extensions in `python/consumers` |
| qname-only | `/tmp/oriole-namespace-qname-only-study` (binaries in `normal/`) | `/tmp/oriole-namespace-qname-only-benchmark/native/native-screen` | `/tmp/oriole-namespace-qname-only-benchmark/python/screen`; extensions in `python/consumers` |

## Reused inputs

- Qualified Oriole runtime: `/tmp/oriole-native-start-end-namespace-declaration-study/normal/liboriole_expat.so`, SHA-256 `c3e6533900cf0b1ec6b127fd173f7e25be210cb5afe23ad9963c84873f6ea025`.
- Normal Expat: `/tmp/oriole-pgo-study/expat-control-liboriole_expat.so`, SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`. The historical directory name does not indicate PGO; this is the normal control.
- Native driver: `/tmp/oriole-grammar-bench-plain/native-driver`.
- Six project XML inputs: `/home/dev-user/code/oss/oriole/benchmarks/projects/corpus`, with `/home/dev-user/code/oss/oriole/benchmarks/projects/corpus-manifest.json`.
- Generated inputs: `/tmp/oriole-text-frame-prototype/text/screen/rare-declarations.xml` and `/tmp/oriole-grammar-bench-plain/entities.xml`.
- Reused qualified-control Python modules: `/tmp/oriole-native-start-end-namespace-declaration-study/python/consumers/candidate`.
- Reused Expat Python modules: `/tmp/oriole-expanded-start-capacity-study/python/consumers/expat`.
- Unmodified CPython sources: `/home/dev-user/.cache/oriole/upstream/cpython-3.12.13`.

## Scope and reproduction

The committed source identities are recorded in the independent reviews. A source-commit record is also copied when the original study supplied one. The build records predate the source commits and retain their original base and patch fields; they have not been rewritten to pretend compilation happened after publication. The mutable development worktree may now contain later experiments.

Controllers/readers are exact originals and retain development-machine paths. Inherited controller captions sometimes use an earlier planned-start experiment name; the preparation record, binding, committed source identity and exact library hashes establish which candidate was measured. Preparation records describe the state before execution; the completed execution records establish that the studies subsequently ran. The scripts are evidence of the protocol, not a relocatable benchmark package.

Each native study contains 28 conditions, seven paired rounds, 672 workers and 106176 total samples. Each completed Python study contains 24 conditions, seven paired rounds, 576 workers and 26364 total samples. All conditions and adverse outcomes remain in the copied full summaries. Two earlier storage test-fixture failures are preserved under `benchmarks/storage/preserved-failures`.

Independent reviews checked saved raw workers and arithmetic without running new parser, compiler or benchmark targets. The benchmark preparer performed those reviews; they are not third-party audits. They do not replace upstream API, strict CPython, sanitizer or fuzz qualification. Canonical comparisons coalesce adjacent character callbacks and do not establish exact callback fragmentation compatibility.

These are shared-host measurements with CPU affinity, not isolated-host or whole-project performance claims. Distinct candidate studies are separate elapsed windows; ratios compare each candidate with the qualified control and Expat within that study.

## Correction to a preserved review note

The original storage `independent-review.json` describes its second failed preparation as expecting two retained cache entries. That description is inaccurate. The preserved raw `tests.log` identifies `arena_start_preserves_raw_context_live_pointers_and_callback_switches`: its `state.starts == 2` assertion failed after an incorrect expectation that the triplet setter changes behavior during parsing. Expat and Oriole ignore that setter after parsing starts. The fixture was corrected before the passing storage build. The raw log and original review remain unchanged; this note corrects the explanation, not the measured results or test outcome.

## Qualification raw evidence

The exact revision-composition qualification remains in `/tmp/oriole-namespace-binding-revision-correctness` and `/tmp/oriole-namespace-binding-revision-asan-study`. The lean alternative has separate runs in `/tmp/oriole-namespace-qname-only-correctness` and `/tmp/oriole-namespace-qname-only-asan-study`. Their complete top-level reports/logs, original API result rows and assertion logs, strict CPython outcomes, compiler proofs and independent sanitizer readbacks are copied under `qualification/`. Instrumented binaries and per-harness corpus files remain local; their hashes and paths are recorded in those reports.

Both compatibility runs preserve 4,349 API passes, 391 failures and zero timeouts, with the same two strict CPython grouping failures. Strict consumers include the existing external-parser cleanup patch; benchmark consumers remain unmodified. Each separate three-harness sanitizer campaign records 45,181 replay executions and explores for 60 seconds per harness. Leak detection is disabled. Source identity is recorded per campaign; results are not transferred to another commit or an installed distribution.

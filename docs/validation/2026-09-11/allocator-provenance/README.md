# Allocator prefix provenance repair

The allocator repair is required for correctness. An `allocator_api2::Box` can
retag its payload pointer so that deriving a pointer to the preceding private
header no longer carries permission to read that header. The retained baseline
Box reproducer fails Miri's Stacked Borrows check; the same single test passes
Tree Borrows. Neither result is hidden.

The repair exposes only initialized header metadata and recovers that header by
address. The header retains the original backing pointer for realloc/free.
Header exposure is renewed after allocation and successful resize, and after a
failed custom realloc that may have touched the header. Payload ownership,
tracking charges, allocator-family callbacks and the public allocator API remain
unchanged. The measured runtime is `e1d263a711e862e4f0f0018b15de9ac83a87a342`,
based on selected Finder; the 70-file manifest changes only `allocator.rs`.

## Correctness and compatibility

- All 143 focused local tests, formatting and strict Clippy pass. Both Miri
  models pass all 34 storage tests at the repaired commit. The raw exposed-
  provenance/integer-to-pointer warnings remain in the archive: Miri checks are
  necessarily incomplete for exposed provenance and do not prove soundness.
- All **4,740 upstream API outcomes** match Finder exactly: 4,347 passes,
  391 assertion failures and two `test_misc_input_2gb` timeouts. The raw API run
  exits 1; resource bounds and failed rows are retained.
- Shared and static CPython 3.12.13 consumers each preserve the complete
  **802-method outcome map**, including the two known callback-grouping failures
  (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`), three expected
  failures and 14 reported skips. Both raw runs exit 2. The same pinned upstream
  allocation-cleanup backport is applied to the consumer on both sides; all six
  upstream test modules remain unchanged.
- Six C consumer runs pass, including 327 allocation scenarios per linkage.
  Saved callback, malformed-input, publication, End and OOM outcomes match their
  baseline. C ASan/UBSan instruments the C consumers, not the Rust PGO library;
  leak checking is disabled. Successful custom-encoding comparison pairs were
  not emitted by the original helper; its commands, counts and zero-difference
  summaries are retained, without claiming reconstruction of those pairs.

## Native elapsed results

Each value is the geometric mean of condition-level medians of seven paired
duration ratios; lower is faster. These are fixed normal and original-generated-G
PGO campaigns against the corresponding Finder and Expat controls.

| Build | Real repair / Finder | Adverse real | Generated repair / Finder | Adverse generated |
| --- | ---: | ---: | ---: | ---: |
| Normal | 0.996917 (−0.31%) | 7/24 | 0.982234 | 2/4 |
| PGO | 1.017200 (+1.72%) | 22/24 | 1.013712 | 4/4 |

PGO repair / PGO Expat is **1.390496** on the 24 real conditions. The correctness
repair is retained despite this measured regression. This is not a causal claim
about instructions or hardware, and there is **no fresh Python elapsed or PBS
distribution-build claim**. Context-frame, BOLT and alternative training changes
are excluded.

The native audit reconstructs all **1,344 workers / 212,352 samples**:
210,840 measured timed samples, 1,176 timed-worker warmups and 336 preflight
samples. It checks callback fingerprints, counts, arguments, return statuses,
sample order, seeded engine/cohort order, every median and paired ratio. All 56
conditions, including adverse generated cases, are in `conditions.csv` and the
archived independent review. The independent build audit binds nine compiler
vectors, fresh original-G profiles and 864 generated-training records.

## Portable evidence

`evidence.tar.gz` contains an index and deduplicated SHA-256 objects. The identical
`archive-index.json` maps original paths to those objects and records hashes for
excluded binaries/build caches. All original study files are represented as
objects, excluded-file identities, or explicit native-worker aliases. The 1,344
individual worker JSON files are byte-exactly reconstructable from the archived
full preflight/results records, avoiding 27.9 MB of duplicated output. The
verifier checks every alias as well as every object. Expat, CPython and Oriole
license/notice files accompany the retained sources.

Using Python 3.12, from any copied package directory:

```sh
python3.12 -I -S verify.py --package . --output readback-new.json
```

This verifies the package manifest, archive/member identities and 70-file source
snapshot, reconstructs native aliases, and runs the unchanged portable native
reader on the six archived protocol/preflight/results JSON files. It loads no
parser and needs no original absolute paths, binaries or XML files. It replays
saved records; it does not repeat the local source/tool/library/XML identity
checks recorded by the original independent audits. The first package readback
is retained as `verification-attempt01.json`.

The archive retains source patches, six original G pipeline helpers, controllers,
compiler/training records, profiles, raw logs, independent reviews and earlier
failed attempts, including the initial reviewer correction for a blank unittest
subtest-parent status. `root-draft.py` preserves the initial packaging proposal;
`package.py` is the final assembler. Original execution scripts contain pinned
local paths and require their documented toolchains, dependencies and upstream
inputs to rerun; the portable verifier is the standalone saved-data entry point.

`report.json` provides the concise result and identity index; `files.json` pins
the package files. No producer evidence was modified during packaging.

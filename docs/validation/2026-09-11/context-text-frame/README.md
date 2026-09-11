# C context Text frame: validation and performance

The Context Text change is selected on the corrected allocator baseline. It
resolves eligible ordinary UTF-8 Text callbacks from the retained C input context
while preserving frame reservation, eager raw-token publication and accounting.
Native PGO improves by 1.38% and Python PGO by 0.30%; normal Python regresses by
0.97%. The complete results and adverse conditions remain part of this decision.

## Measured results

Ratios compare candidate duration with the corrected allocator control; lower is
faster. Each value is the geometric mean of 24 condition-level medians of seven
paired ratios. These fixed campaigns use original XML from six pinned projects,
4 KiB / 64 KiB feeds and the unchanged normal/original-generated-G PGO protocols.

| Consumer | Build | Candidate / control | Change | Slower conditions |
| --- | --- | ---: | ---: | ---: |
| Native | Normal | 0.996488 | −0.35% | 9/24 |
| Native | PGO | 0.986185 | −1.38% | 5/24 |
| CPython | Normal | 1.009705 | +0.97% | 18/24 |
| CPython | PGO | 0.997003 | −0.30% | 12/24 |

Candidate PGO / Expat PGO is **1.370109 natively** and **1.137761 through CPython**.
The four generated native conditions are separate: normal ratio 0.985784, with
none slower; PGO ratio 1.005805, with three slower. These are measurements on a
shared Linux AMD EPYC-Milan host, not a machine-code explanation or a prediction
for other workloads. The six-row main README table is independently reconstructed
in `native-readme-table.json` from 4 KiB, namespace-off raw process medians and
paired ratios; its displayed ratios are not divisions of rounded times.

The native audit checks all **1,344 workers / 212,352 samples** (210,840 timed
measured samples, 1,176 timed warmups, 336 preflight samples). The Python audit
checks all **1,152 workers / 52,728 samples** (51,576 timed measured samples,
1,008 timed warmups, 144 preflight warmups). Both reconstruct callback results,
identities, statuses, seeded process order, every median and all paired ratios.
All 56 native and 48 Python conditions, including every adverse case, are retained.

Python timing uses unmodified CPython 3.12.13 consumers and includes parser
construction, feeding, finalization, callbacks and explicit destruction. Input
reading, imports, canonical validation and `gc.collect` are outside timing;
automatic GC remains enabled. Adjacent text callbacks are coalesced for canonical
checking. The measurements parse original XML files, not complete applications.
Native timing retains its existing namespace-off/on matrix and driver behavior;
no new same-process `XML_Parse` origin probe was added to that legacy driver.

## Exact source and builds

The compiled source is `0f28139f83b04290e62301c38d8ed489f183a88d`, based on allocator
repair `e1d263a711e862e4f0f0018b15de9ac83a87a342`. The 72-file source manifest is
`8a7da275`; normal library `eed1ee24`, PGO library `7cd7c5a8`. All three runtime
files match the preceding source06 candidate. Against the corrected allocator,
there are six intended changed paths: three runtime files, two added unit-test
modules and a comment-only C test-file change. Allocator bytes are exact `e1`.

Fresh normal / generate / use builds preserve all nine actual compiler vectors,
six original Git144 pipeline helpers and 864 generated-training records per
candidate/control. Tools, profiles, library origins and all 76 frozen files were
checked. `allocator-docs-restack.json` separately records the documentation-only
ancestry change to `20eda6ccaa22bd65b0ef1a4b3cf0b15d382a5f33` on `aea523da`; all
72 measured source files remain byte-identical. That restack is not a new build.

## Correctness and compatibility

The exact combined head passes all 16 CI jobs. Four raw Miri jobs verify **45
tests per model**: 34 storage, four core and seven C tests. The integer-to-pointer
and exposed-provenance warnings remain; these checks are incomplete for exposed
provenance and do not prove soundness.

All **4,740 upstream API outcomes** match the corrected allocator byte-for-byte:
4,347 passes, 391 assertion failures and two bounded `test_misc_input_2gb`
timeouts, raw exit 1. Shared and static strict CPython consumers each preserve the
complete **802-method map**, including the two known callback-grouping failures,
three expected failures and 14 reported skips, raw exit 2. These strict consumers
use the same upstream allocation-cleanup backport on both sides; they are distinct
from the unmodified elapsed consumers. All six upstream test modules are unchanged.

Six C consumer runs and 327 allocation scenarios per linkage pass. Saved callback,
malformed, publication, End and OOM observations match the baseline; Expat
callback/position differences remain. The custom-encoding helper reports 36,456
zero-difference comparisons but does not retain successful raw pairs. C ASan/UBSan
covers C consumers, not the Rust PGO library, with leak checking disabled. This
is compatibility parity, not an all-green upstream suite or a new PBS build.

## Retained failures and review roles

The unmodified `historical-local/` packet preserves all six source snapshots,
226 distinct focused local tests, both failed warm-buffer/empty-raw test attempts,
the Clippy comment failure and both first Miri failures. Those results belong to
source06 and its earlier snapshots. The allocator-header defect and the C test
reborrow were then corrected; the current combined `0f` CI success is separately
bound. Historical partial Miri passes are not presented as current success.

The new build reader's first attempt incorrectly expected the CI diff inside
`source.patch`; the evidence correctly stores `non-build-ci.patch` separately.
Its corrected second attempt retains that first failure and resolves relative CI
pin paths. The Python preflight reader's initial missing version-field assertion
is also retained. Neither correction reran a producer or changed a test outcome.
Compatibility, native elapsed and both Python elapsed readers passed first time.

Runtime source and combined CI have separate reviewers. The build readback uses
an established raw-vector/training reader separate from the producer proof, but
shares the candidate build author; it is not independent of controller authorship.
Compatibility, native and Python elapsed data are audited independently of their
collectors. No BOLT, alternative training or deferred-raw change is included.

## Portable evidence

`evidence.tar.gz` contains a SHA-256 object archive; `archive-index.json` is its
identical index with logical paths, original paths, sizes and hashes. Equal files
share objects. Binaries and reusable caches are omitted with their identities.
Expat, CPython, Oriole and historical documentation notices accompany the sources.

Only the **1,344 native worker files** use byte-exact aliases: every original file
was reconstructed from the full embedded native records and byte-compared.
Python row aliases describe JSON values, not whole-file bytes. **Full original
Python report JSON, raw stdout/stderr gzip, worker specs and process records are
retained.** No Python file is omitted on the strength of a JSON-value alias.

From any copied package directory, using Python 3.12:

```sh
python3.12 -I -S verify.py --package . --output readback-new.json
```

The verifier checks all archive objects and the source72 snapshot, reconstructs
native byte aliases, and runs the unchanged native and Python records-only readers
from relocated records. It loads no parser or compiled binary. It compares all
saved arithmetic and Python value-alias records with the original audits. Live
source/tool/XML/binary checks belong to the recorded original audits, not this
portable replay. `verification-attempt01.json` retains the first package readback.

`report.json` contains the machine-readable results and scope; `conditions.csv`
contains every native condition, and the Python audit retains every Python row.
The archive includes source patches, scripts, compiler/training records, profiles,
raw outcomes, first failures and independent reviews. Original execution scripts
still require their pinned toolchains/dependencies and local paths to rerun;
`verify.py` is the standalone saved-data entry point. `files.json` pins the core
package files. Preparation drafts are preserved as historical preparation only.
No producer evidence or repository file was edited during packaging.

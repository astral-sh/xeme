# Compatibility and release gates

## Current source and remaining gates

Runtime `5d983f7e` is selected for a **controlled, opt-in Linux x86-64 CPython 3.12.13 trial**. It combines matched-End raw publication, direct-source Start, dedicated default namespace storage and declaration grammar validation. The [combined report](validation/2026-09-12/native-start-end-namespace-declaration/) and [source binding](validation/2026-09-12/native-start-end-namespace-declaration/source/publication-source-binding.json) verify its 74 tested source files and four auxiliary manifests against normal library `c3e65339`. All seven ordinary commands pass, including 483 Rust tests across 35 result groups. Owned fallbacks, caller allocator routing, limits, sticky failures and callback ownership remain covered. The new public Rust `ErrorKind::TextDeclaration` variant affects exhaustive matches; the C ABI is unchanged.

Confirmation takes **1.1841× Expat’s native real-project time and 1.0558× its CPython time** (ElementTree 1.0914×, pyexpat 1.0214×). Both native and Python meet the current 20% aggregate target in two separate epochs. All 104 epoch-condition rows, 2,496 workers and 265,080 samples are retained, including 12 adverse rows across nine distinct conditions. Individual outliers and generated-workload regressions are not waived; measurements on a shared host do not establish isolation or statistical significance.

The [normal qualification summary](validation/2026-09-12/native-start-end-namespace-declaration/combined/qualification/summary.json) preserves every original API outcome: **4,349 passes, 391 failures and no timeouts** across 4,740 rows; both 2 GiB cases pass under the original three-second alarm. Six C consumers pass. Strict shared/static CPython each retain 802 methods / 809 rendered outcomes and the same two grouping failures (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`, raw exit 2); two separate semantic tests pass per linkage. Strict consumers retain the pinned cleanup backport; timing consumers are unmodified CPython.

Separate 300-row retry and 12-row buffer-tail diagnostics pass with the documented ceiling and observation edits below; they do not replace the original failures or establish exhaustive OOM coverage. All 18 declaration callback outcomes match the qualified standalone source. W3C records 6,003 rows per engine, 4,962 mandatory passes, **960 mandatory failures** and 81 optional observations each. Acceptance matches Expat on all rows. The [960 shared catalog failures](validation/2026-09-12/native-start-end-namespace-declaration/combined/qualification/W3C-classification.md) comprise 954 Fifth Edition name-profile fixtures against Fourth Edition C rules and six version/BOM/declaration cases. The composition preserves 48 diagnostic corrections versus raw-view, while 198 error-code and 1,695 byte-index differences remain. Acceptance does not establish complete callback-payload equivalence or conformance.

[Current-source Rust ASan/fuzz](validation/2026-09-12/native-start-end-namespace-declaration/combined/qualification/asan/readback.json) passes six harnesses, with 69,201 initial inputs, 69,204 replay executions, 8,206,283 exploration executions and no failure artifacts. All 22 commands exited zero and were reaped; nine crate/harness compiler invocations carry instrumentation. Exploration is bounded to 600 seconds per harness, ten seconds per input, 1,536 MiB RSS and 65,536 bytes per input, with leak detection disabled. The original path-typo launch failed before any target and is preserved. [CI](validation/2026-09-12/native-start-end-namespace-declaration/combined/qualification/ci/readback.json) passes 15 ordinary jobs on each of PR169–171, with PGO skipped; both final Miri callback modes pass 15 focused checks.

The [installed PBS trial](validation/2026-09-12/native-start-end-namespace-declaration/combined/qualification/pbs/readback.json) builds generic non-PGO CPython 3.12.13 from 76 source files matching `5d983f7e`. Archive structure, installed identity, 22 custom checks (five skipped) and 1,024 threaded parses on glibc 2.17 pass. The workflow remains failed only on the known grouping methods: host XML records 806 executions/four failures including retries/12 skips; glibc XML records 802 executions/two failures/13 skips. The disclosed cleanup backport is retained, and no installed performance claim is made.

The [final decision](validation/2026-09-12/native-start-end-namespace-declaration/final-adoption.json) accepts these documented callback and diagnostic/position differences for the controlled trial. The performance and bounded qualification milestone is complete. Oriole remains experimental: bounded testing does not establish exhaustive memory safety, complete Expat conformance or full production interchangeability.

## Original failures and diagnostic limits

The [failure census](validation/2026-09-11/api-failure-census/) documents the original 391 assertion failures: 366 allocation retry/schedule expectations, 12 literal Expat version checks, 12 single-buffer resource-policy checks and one deferral allocation-growth assertion. The two original timeout rows were fixed by the preceding outer-whitespace runtime; this candidate preserves all 4,740 selected outcomes. An allocation assertion can stop execution before later semantic assertions; retaining it as a known failure does not prove those unreached assertions.

Earlier allocation diagnostics reached original post-allocation assertions in 72 raised-retry configurations of six tests. A separate 12-configuration buffer-continuation probe reached suspension, callback-clearing, resume and finished-state assertions after the successful empty call. The assertion audit accounts for all 34 nonpassing allocation-test names, and the `4815cf78` rerun retained the same 72/12 scopes. These source-specific diagnostics changed only their documented controls and do not replace the original failure matrix, establish exact callback equivalence everywhere or prove exhaustive allocation-failure coverage. Separate diagnostics pass all 300 retry configurations after raising exactly 25 local retry maxima to 512, and all 12 buffer-continuation configurations after replacing exactly two second-empty-call allocation-result assertions with unconstrained observations. Remaining semantic/tail assertions stay unchanged. Retry probes use 15-second tests, 4 GiB address space and 3 GiB RSS limits; buffer probes retain three seconds, 1 GiB and 768 MiB. These uninstrumented consumers do not fix the original 391 failures or establish exhaustive OOM coverage; the buffer fixture is not a callback-text bytes/count oracle.

## Historical evidence

### Raw-view checkpoint

The following snapshot retains its original source, numbers and then-current gates. Its selected baseline and matched-End candidate references describe that earlier checkpoint.


The selected baseline is runtime `67c704c` (normal library `e59d89d6`). The [matched-End draft report](validation/2026-09-12/matched-native-end-raw/) records the current unselected candidate and its pending Rust ASan/fuzz qualification. The results below apply to the selected baseline.

The [shared native/raw Text report](validation/2026-09-12/shared-native-raw-text/) binds the selected nine-file raw-view change to PR165 base `2c4d216` and normal library `e59d89d6`. Native UTF-8 root input also supplies Expat input context; eligible raw Text uses a checked range into that owner. The range is bounded by the existing 4 KiB eligibility and participates in input retention. Conversion, partial/invalid UTF-8 and explicit raw overrides keep owned fallbacks. Allocator routing, structural/live limits, callback reentry guards and frame reservations remain intact; allocation schedules and retained capacity can change. The later [source commit binding](validation/2026-09-12/shared-native-raw-text/source/published-source-binding.json) records `67c704c123b8661a3ad1f91bd48c2f0be4996096` with all tested bytes unchanged; tests preceded that commit. Its ordinary seven-command build passes 475 Rust tests across 35 groups, formatting and strict workspace/benchmark Clippy; two test-fixture corrections and their failed attempts are retained.

The complete original API matrix records **4,349 passes, 391 failures and no timeouts**, with all 4,740 ordered outcomes identical to the selected control. Both original 2 GiB cases retain their passing result under the unchanged three-second alarm. Six C consumers pass. Strict shared/static CPython each retain 802 methods, 809 rendered outcomes and the same two grouping failures (raw exit 2); two separate semantic checks pass per linkage. Strict consumers use the pinned upstream cleanup backport; benchmark modules use unmodified CPython sources.

Confirmation takes 0.9776412× selected native real-file time and 0.9837734× selected CPython time, remaining 1.2659× and 1.0975× Expat. ElementTree is 1.1423× and pyexpat events 1.0544× Expat. The Python aggregate and both consumer aggregates meet the roughly 1.20× target; native remains above it. Individual outliers remain in the reports. Initial measurements remain separate. Four epochs retain 104 conditions, 2,496 workers, 265,080 samples and all four adverse conditions. Shared-host affinity and counters establish neither isolation nor statistical significance.

The [current supplemental records](validation/2026-09-12/shared-native-raw-text/qualification/) pass 300 raised-ceiling retry configurations and 12 buffer-tail configurations, with their exact edits and remaining limits disclosed below. They do not change the original 391 failures. The [W3C classification](validation/2026-09-12/shared-native-raw-text/qualification/W3C-classification.md) records identical acceptance on 6,003 rows per engine, but a failed conformance run: each engine has 4,962 mandatory passes, 960 mandatory mismatches and 81 optional observations. Of the mismatches, 954 rows are explicitly Fifth Edition name fixtures, whereas both C interfaces select Fourth Edition name rules; six other mismatches concern external-entity versions and BOM/encoding declarations. Error codes, positions and child outcomes can still differ. No failures are waived.

Six Rust ASan/fuzz harnesses pass for selected baseline `67c704c`: 69,199 replay executions and 7,978,060 exploration executions, with no recorded failure artifacts. [Completed ASan evidence](validation/2026-09-12/shared-native-raw-text/qualification/asan/) records nine instrumented crate/harness invocations and 22 successful, reaped commands. Leak detection is disabled; bounded exploration is not exhaustive safety or installed-distribution validation.

The [current installed PBS trial](validation/2026-09-12/shared-native-raw-text-pbs/) completed on PR166 head `47776625`, containing runtime `67c704c`: archive structure, installed identity, custom checks and 1,024 threaded parses on glibc 2.17 pass, while both installed XML suites retain the two known grouping failures and the workflow remains failed. Host retries repeat both failures (806 executions, four failure executions); glibc records 802 executions and two failures. This is installed compatibility evidence, with no installed-speed claim.

Final CI at `008d818` passes 15 ordinary jobs, including both Miri modes, with PGO skipped; its later CI-inventory and test-fixture fixes leave production bytes unchanged and were not part of the PBS execution.

Earlier-source sanitizer and installed-distribution evidence remains historical; ABI, acceptance, callback behavior, resource safety and installed-consumer validation are distinct gates.

### Earlier checkpoints

The preceding [outer-whitespace runtime `6320d7b7`](validation/2026-09-12/outer-whitespace-separated-dispatch/) has separate sanitizer and installed-consumer evidence. C ASan/UBSan covers the six C consumers linked to uninstrumented Rust, with leak detection disabled. Strict CPython uses an explicit upstream cleanup backport; benchmark consumers use unmodified CPython sources. That preceding source’s Rust ASan passes all six harnesses: 69,199 replay executions and 8,194,533 exploration executions. Nine required Rust crate/target invocations are ASan-instrumented; normal generic libraries are provenance controls only. Each harness uses a 600-second exploration bound, a ten-second input timeout, 1,536 MiB RSS limit and 65,536-byte maximum input, with leak detection disabled. All 22 recorded commands exited zero and their children were reaped. This bounded campaign does not establish exhaustive memory safety or installed-distribution compatibility. The first launch failed before fuzz compilation because its PATH omitted cargo/bin, so cargo-fuzz could not invoke bare rustc; all child processes were reaped and all six harnesses remained unstarted. The retry changes only the output path and fixes the launch PATH, retaining the runner, source, target directory, flags, six harnesses and bounds. The [normal generic PBS trial](validation/2026-09-12/pbs-current-normal/) validates this runtime's archive and completes 1,024 threaded parses on glibc 2.17; its installed XML suites retain the two known grouping failures. ABI coverage, well-formedness, callback behavior, resource safety and installed-consumer validation remain distinct gates.

The linked reports retain their exact sources, normal-build comparisons, all adverse rows and original outcomes. Buffered, empty-state, eager-position, attribute, LF and namespace results are historical controls rather than validation of this candidate. The buffered arena diagnostic used one 4 MiB feed, retained broad Expat variability and was excluded from its four main campaigns; follow-up proposals mentioned there were unbuilt at that checkpoint. Earlier `4064b065` ASan, glibc 2.17 and installed v3/PBS checks apply to that source only. Its strict installed run had 802 methods and two grouping failures; retries yielded 806 executions/four records for those same methods. The historical normal/PGO/v3 results are retained evidence, not current-source optimized measurements or authorization for new PGO work. Earlier streaming-policy records separately cover 257 MiB streams and a 2,049 MiB text stream.

- [buffered-delivery report](validation/2026-09-12/buffered-delivery/)
- [empty-state guard report](validation/2026-09-12/empty-state-guards/)
- [eager bare-tag position report](validation/2026-09-12/eager-bare-tag-positions/)
- [quoted-attribute lane-mask report](validation/2026-09-12/attribute-lane-masks/)
- [SIMD Text lane-mask report](validation/2026-09-12/text-lane-masks/)
- [sparse namespace storage report](validation/2026-09-12/sparse-namespace-storage/)
- [benchmark and compatibility evidence](validation/2026-09-11/reference-frames/)
- [sustained ASan and PBS validation](validation/2026-09-11/reference-validation/)
- [installed v3 distribution trial](validation/2026-09-12/v3-distribution/)
- [earlier streaming report](validation/2026-09-11/streaming-input-bounds/)
- [earlier integration report](validation/2026-09-10/start-frame-integration/)
- [full checkpoint](validation/2026-09-10/)
- [exact failure census](validation/2026-09-11/api-failure-census/)
- [earlier-source allocation diagnostic](validation/2026-09-11/allocation-semantic-coverage/)
- [buffer-state diagnostic and assertion audit](validation/2026-09-11/buffer-state/)
- [fresh rerun](validation/2026-09-12/fused-attribute-normal/)
- [optional x86-64-v3 experiment](validation/2026-09-11/reference-v3/)

## Differential testing

```console
python3 tools/differential.py --library /absolute/path/liboriole_expat.so \
  --output /tmp/oriole-differential
```

The reference is the host's `libexpat`, whose version is recorded. Override it with
`--reference /absolute/path/libexpat.so`. Each implementation runs in a subprocess
with a timeout. The report retains every input as base64 and SHA-256, the random
seed, chunk sizes, callbacks, status, error code, and final line/column/byte index.
Generated valid documents and deterministic byte mutations supplement the named
corpus. Use `--generated 1000` for a longer run.

The semantic gate compares acceptance, exact error codes, and callbacks with only
adjacent text fragments coalesced. The exact gate (`--strict`) also compares text
fragmentation and final locations. Every difference remains in the report; neither
gate silently accepts mismatches. Coalescing is a useful semantic comparison but
cannot establish compatibility for consumers sensitive to callback boundaries.
No expected-failure list is used to turn uncovered behavior into a passing test.

The named corpus covers declarations, comments, processing instructions, CDATA,
XML names, attributes, newline normalization, references, entities, DTD attribute
defaults, namespaces, UTF-8, UTF-16, and Latin-1. Invalid documents cover malformed
names, UTF-8, numeric references, attribute syntax, entity recursion, reserved
namespaces, truncation, and misplaced markup. Resource exhaustion and callback
lifecycle probes are additional tests, not ordinary differential assertions:
Oriole's documented resource ceilings intentionally differ from Expat's defaults.

Custom-encoding probes additionally distinguish converted ASCII from raw markup,
reference syntax, name spellings, namespace separators, whitespace, and DTD
keywords. The [provenance checkpoint](validation/2026-09-10/custom-encoding-provenance/)
records the full upstream API results, original byte templates, converter maps,
chunk sizes, successful semantic callbacks, and remaining default-handler and
diagnostic differences. Its bounded differential grids do not establish complete
Expat compatibility.

The C interface selects XML 1.0 Fourth Edition name rules to match Expat;
the Rust interface defaults to Fifth Edition and exposes `Config::name_rules`.
Name validation uses the selected edition in element and attribute names, DTD
grammar, entity references, incremental scanners, and external children.
Custom-encoding PUBLIC identifiers classify the original bytes with that same
edition before reporting decoded callback text.

## Streaming resource limits

The C interface accepts at most 256 MiB in one input call or buffer request. Each
original source, including an external child's source, can accept cumulative input
up to `min(c_long::MAX, isize::MAX)` so byte positions and one-based line counts
remain representable. The former 256 MiB lifetime C input ceiling no longer
rejects longer ordinary incremental streams.

Cumulative indirect work is bounded by `max(8 MiB, 100 × consumed root bytes)`;
cumulative event payload is bounded by `max(64 MiB, 100 × consumed root bytes)`.
Only consumed original root input supplies credit. Buffered suffixes, external
input and reset documents cannot subsidize work in the earlier document. Children
retained across a root reset keep the original budget. Checked counters reject
overflow even when the relative threshold saturates.

The 512 MiB live/reserved family-allocation ceiling and existing allocation and
entity amplification checks remain independent. Token, attribute, depth, entity,
cycle, external-depth and child-construction limits still apply. Expat-compatible
amplification setters do not disable the fixed C work policy or live ceiling.
The Rust interface keeps its default 256 MiB cumulative input and absolute 8 MiB
work limits; `Limits.max_work_amplification` defaults to `None` and explicitly
opts into relative work when set. See the
[policy and arithmetic audit](validation/2026-09-11/streaming-input-bounds/POLICY.md)
for accounting and source-position invariants.

## Native consumer tests

`tests/c/integration.c` compiles against the public header and exercises:

- Agreement between the version string and numeric compatibility revision.
- One-byte incremental input, parser finalization, and reset.
- The public `XML_GetUserData` macro, whose ABI reads the parser's first field.
- `XML_GetBuffer` and `XML_ParseBuffer`.
- Callback suspension and resumption.
- Custom memory allocation, reallocation, freeing, and initial allocation failure.
- Parser pointers as callback arguments.
- Custom conversion callbacks with ASCII aliases, original-byte end-tag identity,
  decoded duplicate attributes, incremental buffer input, and encoding release.

Every invocation includes the custom memory suite and initial allocation failure
gates. Parser storage uses fallible allocator-aware containers. Allocation failure
must return a null parser or `XML_ERROR_NO_MEMORY`, release all successfully
allocated blocks, and leave a failed `XML_MemRealloc` block available to its owner.

`tests/c/adversarial.c` independently probes callback-time parser deletion,
same-parser reentry rejection, recursive default-handler rejection, independent
parsers in callbacks, and alias-safe base replacement. Following Expat 2.8.4,
freeing an actively parsing parser from its callback is ignored; the caller frees
it after the outer call returns. Forbidden nested parsing, buffer, reset, and
resume calls fail without poisoning the outer parse. Separate guards protect
recursive encoding-release callbacks and allocator callbacks.

Compile the same integration source against both libraries. A reference run must pass before
its assertions are used to judge Oriole. The harness was bootstrapped against
system Expat 2.6.1. These C allocation counts validate public ownership. Separate
Rust tests inject failure at each allocation, detect allocations escaping the
supplied suite, force reallocations to move across alignment offsets, and exercise
concurrent shared ownership. The allocation tracker counts live backing bytes,
including adapter metadata, across a parser family. Resizing and freeing a block
use the tracker owned by that block, including after the parser that created it
has been freed. A 512 MiB ceiling on live backing allocations applies to each
parser family, independently of its configured amplification factor and threshold.
Crossing this ceiling returns `XML_ERROR_NO_MEMORY`.

## CPython integration

CPython must execute its actual extension and XML test suites against the candidate
library. Replacing the Python-level module with a simulation would bypass the
consumer's callback, ownership, capsule, and error-location contracts.

The [CPython 3.12.13 consumer](https://github.com/python/cpython/blob/v3.12.13/Modules/pyexpat.c)
requires more than `XML_Parse`:

| Area | Required behavior |
| --- | --- |
| Construction | `XML_ParserCreate_MM`, a caller-supplied memory suite, namespace separator, encoding, hash salt |
| Event delivery | All element, text, namespace, CDATA, declaration, DTD, entity, default, and skipped-entity callbacks |
| DTD models | `XML_Content` layout, recursive model ownership, and `XML_FreeContentModel` |
| Input | `XML_GetBuffer`, `XML_ParseBuffer`, `XML_GetInputContext`, custom encoding maps |
| State | Correct callback stop behavior, base URI, specified attribute count, namespace triplets |
| External entities | Child parser construction, callback return values, context, and parent lifetime |
| Introspection | Error constants/strings, positions, version structure, feature list |
| Integration | The `pyexpat` C capsule used by `_elementtree`, including identical callbacks and allocator ownership |

The [earlier-source streaming consumer result](validation/2026-09-11/streaming-input-bounds/#cpython-consumer-checks)
uses an allocation-failure cleanup backport in `Modules/pyexpat.c`, with unchanged
upstream tests. Shared and static linkage each preserve all 802 method outcomes
from the earlier unmodified-consumer control: the same two failures, three
expected failures and 14 reported skip records. Those skips comprise five methods,
eight subtests and one class setup. Both suites exit 2; they are not green suites.
Four initial/fresh `pyexpat` and `_elementtree` origin records are checked per
linkage. The report keeps the new patched consumer distinct from the earlier
unmodified consumer and does not claim a full distribution build.

The [upstream pyexpat tests](https://github.com/python/cpython/blob/v3.12.13/Lib/test/test_pyexpat.py)
are one gate. ElementTree, SAX, minidom, and pulldom exercise separate consumers and
must also pass. Supported CPython versions need separate builds and reports.
The executable integration harness lives in `tools/cpython/`. It pins CPython
3.12.13 at `3bb231a6a5dc02b95658877318bf61501a7209e9`, builds both `pyexpat` and
`_elementtree`, and verifies their loaded paths in every test worker. This check
matters for PBS interpreters that normally load built-in versions before modules
on `PYTHONPATH`.

```console
python3.12 tools/cpython/run.py --source /path/to/cpython-3.12.13 \
  --library /path/to/liboriole_expat.so --output /tmp/oriole-cpython
```

The source checkout must be clean and the interpreter must be exactly 3.12.13.
The optional `--system-allocator` flag adapts the consumer's construction calls;
its results measure that explicit adaptation and cannot pass the unmodified
consumer gate. The report retains source, compiled-source and library hashes,
compile commands, module origins, and complete upstream test output.

## python-build-standalone

At audited revision
[`a4553880293fe9d1bb62747d34ab0e5121d3554f`](https://github.com/astral-sh/python-build-standalone/blob/a4553880293fe9d1bb62747d34ab0e5121d3554f/cpython-unix/build-expat.sh),
PBS builds Expat as a static, position-independent library for the target toolchain
and installs it under `/tools/deps`. Replacing this dependency therefore requires
cross-compilable Rust static archives, matching headers and native link metadata,
and the platform's existing deployment-target requirements. A local shared-library
probe alone does not satisfy that packaging gate.

Before a production substitution, retain evidence for the PBS-built interpreters
on every supported platform and architecture, including static linking, extension
loading, shared allocator ownership, and the complete XML consumer suites. Also
run upstream Expat compatibility tests, sanitizer-backed C callback probes, parser
fuzzing, independent adversarial review, and reproducible benchmarks. Passing a
bounded local corpus is evidence of the behaviors tested, not a full replacement
claim.

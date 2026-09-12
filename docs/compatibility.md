# Compatibility and release gates

Oriole targets the ordinary narrow-character Expat C ABI. ABI coverage,
well-formedness, callback compatibility, and safe resource use are separate gates.
A symbol existing or a document parsing successfully does not establish callback
or CPython compatibility.

The [quoted-attribute lane-mask report](validation/2026-09-12/attribute-lane-masks/) binds the selected `91038723` runtime to its normal build and two separate native/CPython measurement epochs. Confirmation observes 2.39% less real-project native time and 1.60% less CPython time against the preceding LF runtime, remaining 1.3886× and 1.1535× Expat. Five of 24 CPython conditions meet the roughly 1.10× goal; both aggregates remain above it. Native improves in 21/24 real conditions and CPython in 20/24. All three native real-input, two generated and four Python confirmation regressions remain explicit; generated time increases 3.08%, with the two entity cases increasing 7.37% and 10.04%. Shared-host counter observations qualify these windows without establishing isolation or statistical significance.

Current-source checks pass 456 Rust tests across 35 groups and independently preserve all 4,740 original API configurations and 802 strict methods per linkage: 4,347 API passes, 391 assertion failures, two timeouts and the same two strict callback-fragmentation failures (809 rendered outcomes/raw exit 2). Six C consumers and two separate semantic tests per linkage pass. Strict consumers contain the explicit CPython cleanup backport; benchmark consumers are unmodified. C ASan/UBSan harnesses link uninstrumented Rust with leak detection disabled. Dependencies are unchanged from LF; current-source sustained fuzzing and an installed PBS trial remain outstanding.

The [SIMD Text lane-mask report](validation/2026-09-12/text-lane-masks/) binds the preceding LF source to `cacc0b1b`. Its normal-build confirmation takes 1.4248× Expat's native time and 1.1745× its CPython time, observed reductions of 1.55% and 1.21% against the preceding namespace runtime. Four of 24 CPython conditions meet the roughly 1.10× goal; both aggregates remain above it. Native improves in 22 real-input conditions and CPython in 23. The two native real-input, one generated and one Python confirmation regressions remain, alongside all separate initial measurements and the rejected fixed16 experiment. Before/during host counters qualify the shared-host window but do not establish isolation or statistical significance.

That LF source passes 455 Rust tests across 35 groups and preserve all 4,740 original API outcomes and 802 strict CPython method outcomes per linkage: 4,347 API passes, 391 assertion failures and two timeouts; the same two strict callback-grouping failures remain, with 809 rendered outcomes per linkage and each strict suite exiting 2. Strict consumers use the explicit upstream allocation-failure cleanup backport; benchmark consumers are unmodified. Six C consumers and two supplemental semantic tests per linkage pass. C harnesses use ASan/UBSan with uninstrumented Rust release libraries and leak detection disabled. The three new dependency archives and all 156 source files, licenses and resolved features were reviewed; metadata does not establish tested MSRV or every architecture. Current-source sustained fuzzing and an installed PBS trial remain outstanding.

The preceding [sparse namespace storage report](validation/2026-09-12/sparse-namespace-storage/) retains its own `b9190cda` source, smaller Element layout, compatibility gates and two measured epochs.

The previous `4064b065` source has separate [benchmark and compatibility evidence](validation/2026-09-11/reference-frames/), [sustained ASan and PBS validation](validation/2026-09-11/reference-validation/) and an [installed v3 distribution trial](validation/2026-09-12/v3-distribution/). Those source-specific sanitizer, glibc 2.17 and installed-consumer results do not validate the new candidate.

The [earlier streaming report](validation/2026-09-11/streaming-input-bounds/)
records the latest input/work-policy validation, including actual streams through
257 MiB and a separate 2,049 MiB text stream. The
[earlier integration report](validation/2026-09-10/start-frame-integration/) and
[full checkpoint](validation/2026-09-10/) retain their own source-specific runtime,
fuzzing and distribution evidence.

The current candidate preserves the original API matrix outcomes from `4064b065`: 4,347 of 4,740 configurations pass; 391 assertion failures and two timeouts remain, spanning 38 test names. The [exact failure census](validation/2026-09-11/api-failure-census/) retains every result and reached source assertion:

| Reached failure | Configurations |
| --- | ---: |
| Allocation retry ceilings or allocation schedules | 366 |
| Literal Expat version identity | 12 |
| Single-buffer resource policy | 12 |
| Deferral allocation-growth assertion | 1 |
| Three-second timeout on a 2 GiB case | 2 |

These failures do not establish a new XML-content defect, but an early allocation assertion does not validate later semantic assertions that were never reached. A separate [earlier-source allocation diagnostic](validation/2026-09-11/allocation-semantic-coverage/) passes all 72 configurations of six tests after raising retry and resource limits. It reaches their original text and handler assertions without changing the original matrix or declaring those failures fixed. Callback presence checks do not establish exact argument values, ordering or positions.

A further [buffer-state diagnostic and assertion audit](validation/2026-09-11/buffer-state/) accounts for all 34 nonpassing allocation-test names. The six earlier cases exhaust their post-loop text and handler-flag assertions. A separate copy of `test_nsalloc_parse_buffer` records its successful empty call, then passes all original suspension, callback-clearing, resume and finished-state assertions in 12 configurations at the original limits. Neither diagnostic replaces the original allocation-schedule failures or proves exhaustive allocation-failure coverage. A [fresh rerun](validation/2026-09-12/fused-attribute-normal/) on the preceding `4815cf78` runtime passes the same 72 raised-retry and 12 buffer-continuation configurations; it is separately scoped from the current candidate's unchanged original matrix.

The previous `4064b065` [normal/PGO benchmark report](validation/2026-09-11/reference-frames/) and [optional x86-64-v3 experiment](validation/2026-09-11/reference-v3/) retain their own performance and unchanged API/strict-suite outcomes. Their optimized results have not been remeasured on the current candidate.

The [stable v3 PGO distribution trial](validation/2026-09-12/v3-distribution/) also uses the previous source. It verifies the installed target, manifest and optimized static archive, and passes the glibc 2.17 identity and 1,024-threaded-parse probe. Its XML suite reports 802 tests and two callback-grouping failures; host retries produce 806 executions and four failure records for those same two methods. Supplemental checks preserve text, callback-controlled buffering and element/CDATA order in that installed interpreter and separate shared/static consumers. These results do not establish installed performance or validation of the new source.

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

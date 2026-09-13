# Compatibility and release gates

Oriole is experimental. The C interface targets Expat 2.8.4's narrow-character
ABI, with an opt-in Linux x86-64 CPython 3.12.13 integration. It is not a complete
production replacement for Expat.

## Known differences

- Text callback fragmentation can differ, including the CPython assertions in
  `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Coalescing adjacent
  text can establish semantic agreement without satisfying these consumers.
- Some malformed-input errors, byte positions, and external-DTD default-handler
  prefixes differ, including with custom encodings.
- The C interface uses XML 1.0 Fourth Edition name rules, matching the pinned
  Expat reference. The Rust API defaults to Fifth Edition. This affects W3C
  corpus fixtures that require Fifth Edition names.
- Like the reference Expat build, Oriole can accept external entities declaring
  XML 1.1 and documents combining a UTF-8 BOM with an ISO-8859-1 declaration,
  although the W3C catalog marks those cases as not well formed.
- Allocation schedules, resource ceilings, and reparse-deferral allocation growth
  differ from Expat. Upstream tests that assume its allocation counts or retry
  schedule can fail before reaching later semantic assertions.
- `XML_ExpatVersion()` identifies Oriole and its Expat API target, so literal Expat
  version-string checks fail.
- Wide-character and `XML_LARGE_SIZE` builds are unsupported. Namespace separators
  must be ASCII. External value children have encoding-declaration restrictions
  described in the [C interface guide](../crates/oriole_expat/README.md).

CPython's external-parser allocation cleanup needs the explicit
[upstream backport](../integration/python-build-standalone/consumer-fix/README.md)
for the pinned consumer version. The backport changes consumer ownership handling;
it does not resolve the callback differences above.

## Release gates

Production substitution requires XML conformance, Expat API and callback
compatibility, CPython XML consumers, bounded resource use, sanitizer and fuzz
coverage, and installed-distribution checks on every supported platform. Passing
local tests or coalesced callback comparisons does not establish all of these
contracts. Retain failing outcomes when comparing a change with its parent;
allocation diagnostics with altered retry ceilings must remain separate from the
unmodified upstream tests.

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
keywords. Include original byte templates, converter maps, and chunk sizes in
regressions so decoding does not erase the syntax distinction being tested.

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
remain representable.

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
opts into relative work when set.

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

Compile the same integration source against both libraries. A reference run must
pass before its assertions are used to judge Oriole. These C allocation counts
validate public ownership. Separate Rust tests inject failure at each allocation,
detect allocations escaping the supplied suite, force reallocations to move
across alignment offsets, and exercise
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

At the integration's pinned revision
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

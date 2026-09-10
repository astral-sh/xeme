# Compatibility and release gates

Oriole targets the ordinary narrow-character Expat C ABI. ABI coverage,
well-formedness, callback compatibility, and safe resource use are separate gates.
A symbol existing or a document parsing successfully does not establish callback
or CPython compatibility.

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

## Native consumer tests

`tests/c/integration.c` compiles against the public header and exercises:

- One-byte incremental input, parser finalization, and reset.
- The public `XML_GetUserData` macro, whose ABI reads the parser's first field.
- `XML_GetBuffer` and `XML_ParseBuffer`.
- Callback suspension and resumption.
- Custom memory allocation, reallocation, freeing, and initial allocation failure.
- Parser pointers as callback arguments.

The default mode is the full gate, including custom memory suites. Oriole currently
rejects non-null custom memory suites: the Rust core's allocations cannot yet obey
a per-parser C allocator contract. `--supported` runs only the implemented lifecycle
and buffer subset; `--unsupported-mm-contract` checks explicit rejection. Neither
mode passes the full replacement gate.

`tests/c/adversarial.c` independently probes Oriole's callback-time parser deletion,
same-parser reentry rejection, recursive default-handler rejection, independent
parsers in callbacks, and alias-safe base replacement. Callback-time parser deletion
is an Oriole guarantee; this test is not presented as equivalent Expat behavior.

Compile the same integration source against both libraries. A reference run must pass before
its assertions are used to judge Oriole. The harness was bootstrapped against
system Expat 2.6.1. Custom allocation counts here validate public memory ownership;
they do not prove that every internal Rust allocation uses the supplied suite.

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

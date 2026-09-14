# Compatibility

Xeme is experimental. CPython consumer checks use pinned CPython 3.12.13 sources.
Some callback boundaries and error positions differ from Expat.

[Test reports](evidence/README.md) record results for specific revisions.

## Interface scope

- The safe Rust interface defaults to XML 1.0 Fifth Edition names. The C interface
  selects Fourth Edition names to match Expat; both are nonvalidating parsers.
- The C ABI supports narrow characters, namespaces, DTD processing, custom memory
  suites, incremental and buffer input, reset, suspension and external children.
  Wide-character, `XML_LARGE_SIZE` and `XML_ATTR_INFO` configurations are rejected.
  Namespace separators must be ASCII.
- Applications supply external resources. The parser performs no filesystem or
  network I/O. External-entity policy belongs to the embedding application.
- The [C interface guide](../crates/xeme_expat/README.md) describes pointer
  lifetimes, custom encodings and unsupported behavior.

## Known semantic differences

Some malformed-input errors, byte positions and external-DTD default-handler
prefixes differ, including with custom encodings. External value children have
encoding-declaration restrictions described in the [C guide](../crates/xeme_expat/README.md).
Like the pinned Expat build, the C interface accepts some external XML 1.1
declarations and UTF-8 BOM/ISO-8859-1 combinations that the W3C catalog rejects.
The native Rust parser rejects a UTF-8 BOM that conflicts with the declared
encoding unless an explicit higher-level encoding override applies.
`Config::allow_utf8_bom_encoding_mismatch` defaults to `false`; the C adapter
explicitly enables this legacy compatibility option.

The pinned CPython consumer needs the
[allocation-cleanup backport](../tools/cpython/consumer-fix/README.md).

## Streaming resource limits

The C interface has these fixed limits:

| Resource | C interface limit |
| --- | --- |
| One input call or buffer request | 256 MiB |
| Cumulative original input, per source | `min(c_long::MAX, isize::MAX)` |
| Cumulative indirect work, per parser family | `max(8 MiB, 100 × consumed root bytes)` |
| Cumulative event payload, per parser family | `max(64 MiB, 100 × consumed root bytes)` |
| Tracked live/reserved parser backing allocations, per family | 512 MiB |
| External-child construction | 1,024 attempts (including failures), ancestry depth 32 |
| Declared entities / internal expansion depth | 100,000 / 100,000 |

Token, attribute, element-depth and entity-cycle limits also apply. Work allowances
grow with consumed root input. Buffered input, external input and input after a
reset do not increase the allowance for earlier work. Children retained across
root reset keep the original work, callback and child counters. Allocation
tracking remains shared across reset, including its reset input denominator.
Checked counters reject overflow.

Expat-compatible entity and allocation amplification setters do not disable the
fixed work policy or live allocation ceiling. Application-owned blocks requested
through `XML_MemMalloc` and `XML_MemRealloc` use the selected allocator but are
exempt from parser amplification accounting. Allocation headers retain their
tracker through reallocation and destruction of the originating parser.

The Rust interface defaults to 256 MiB cumulative input, 16 MiB tokens, 256 nested
elements, 10,000 declarations, 32 entity levels and 8 MiB cumulative work.
`Limits.max_work_amplification` defaults to `None`; setting it explicitly enables
relative work. Importing an unrelated DTD charges new declaration bytes and
structure to the recipient and copies strings and retained declaration bases
through its allocator. See the [Rust guide](library.md) for configuration.

## Native Rust conformance gate

CI checks the safe Rust parser directly with
[`tools/w3c/native.py`](../tools/w3c/native.py), independently of the C interface
and Expat. The gate uses XML 1.0 Fifth Edition names, each catalog case's
Namespaces 1.0 mode, and the native parser's default resource limits. Its oracle
is the pinned W3C catalog interpreted under Fifth Edition, with no
accepted-failure baseline.

All 6,003 selected rows must run, covering chunks of 1, 7 and 4,096 bytes and
including 81 optional observations. As a nonvalidating parser, Xeme must accept
both `valid` documents and `invalid` documents containing DTD validity errors;
it must reject `not-wf` documents after the edition correction below. Catalog
`error` cases permit either outcome.
XML 1.1 and older-edition-only cases are recorded with their exclusion reasons.
The checked-in corpus manifest binds the selection and suite bytes: incomplete
or changed inputs, resolver failures, and worker failures reject the gate.
Resource-limit and allocation failures are inconclusive and also fail the gate;
they cannot count as successful rejections of malformed input.

One catalog expectation needs a Fifth Edition correction: `rmt-e2e-38` labels
an external entity with a `1.1` declaration as `not-wf`, although its content
uses only XML 1.0 features. [W3C erratum E10](https://www.w3.org/XML/xml-V10-4e-errata#E10)
reversed the earlier rule, so the native gate requires acceptance in all three
chunk sizes. The manifest binds this specification-based correction alongside
the corpus inventory. Reports retain the original catalog descriptor and raw
catalog mismatches; the case remains mandatory and the selection stays at
6,003 rows.

External entities use a local-only resolver confined to the pinned suite, with
2 MiB files, depth 32 and at most 1,024 requests. This is a standard acceptance
gate, not DTD validation or canonical-output conformance. See the
[W3C harness guide](../tools/w3c/README.md) for commands and report scope.

## Expat compatibility regression gates

CI runs four pinned regression suites through
[`tools/compatibility.py`](../tools/compatibility.py). CI allows known failures
only when they match the checked-in baseline.

| Suite | Checks |
| --- | --- |
| Expat 2.8.4 API | All 395 tests in 12 configurations: 4,740 rows. Expat must pass every row. Candidate failures must match the checked-in configuration and assertion baseline. |
| Allocation behavior | All 83 public allocation-suite tests plus the deferral-growth test in 12 configurations: 1,008 reported rows per engine. Both engines must pass the adapted tests and their ownership checks, with no failure allowance. |
| W3C XML catalog through the C interface | All 6,003 selected rows per engine, including 81 optional observations. Verify selected tests, loaded bytes and namespace mode; compare acceptance and child outcomes. |
| Differential corpus | Named fixtures plus 200 deterministic generated cases. Verify every worker ran and compare callbacks and error codes; exact text fragmentation and final positions have a separate strict mode. |

The API baseline retains 509 failures: 484 allocation retry/schedule assertions,
12 literal version checks, 12 single-buffer policy checks and one deferral-growth
assertion. An early allocation assertion can hide later semantic assertions.
The [rebase check](evidence/2026-09-13-rebase.md) records the latest baseline update.
The separate [allocation-behavior gate](evidence/2026-09-13-allocation-behavior.md)
raises retry ceilings to 512 and adapts allocation-count assumptions while
retaining callback, error, state-transition and cleanup checks. The deferral test
exercises 504 size combinations in its one active configuration; its other 11
configurations return early. Some fixtures check successful parsing without
comparing complete event data. The original 509 failures remain in the API baseline.

Candidate tests retain three-second and 1 GiB address-space limits. Original API
reference tests use 30 seconds and 4 GiB so their large-buffer cases can complete;
allocation-behavior reference tests use the candidate limits. All keep a 768 MiB
RSS cap. Commands and limits remain in the raw reports.

The C-interface W3C baseline retains 960 mandatory failures in both engines:
954 Fifth Edition name-profile rows and six version/BOM/declaration rows. These
are C-interface results; the native Rust gate independently requires Fifth
Edition acceptance. Matching Expat does not establish conformance to that
catalog. New failures, missing configurations, changed assertions, resolver
failures and corpus changes fail CI;
improvements are reported separately. Baseline changes require review.

For a local run, use a fresh output directory and the pinned sources from CI:

```console
python3 tools/compatibility.py api --library /absolute/libxeme_expat.so \
  --reference /absolute/libexpat.so --source /absolute/expat-2.8.4 \
  --config /absolute/expat-build/expat_config.h --output /tmp/xeme-api
python3 tools/compatibility.py allocation --library /absolute/libxeme_expat.so \
  --reference /absolute/libexpat.so --source /absolute/expat-2.8.4 \
  --config /absolute/expat-build/expat_config.h --output /tmp/xeme-allocation
python3 tools/compatibility.py w3c --library /absolute/libxeme_expat.so \
  --reference /absolute/libexpat.so --source /absolute/xmlconf --output /tmp/xeme-w3c
python3 tools/compatibility.py differential --library /absolute/libxeme_expat.so \
  --reference /absolute/libexpat.so --output /tmp/xeme-differential
```

## CPython and distribution checks

[`tools/cpython`](../tools/cpython/README.md) pins CPython 3.12.13, builds its
`pyexpat` and `_elementtree` extensions, and verifies loaded module origins. All six
unchanged upstream XML suites must pass. The C interface preserves line-break
callback boundaries used by `BufferTextTest.test1` and
`CDATAHandlerTest.test_handlers`; the [test report](evidence/2026-09-13-cpython-grouping.md)
records their resolution. The harness also checks pinned fixture hashes,
the complete list of discovered and executed tests, and semantic checks on those inputs.
The allocation-cleanup backport is separate from upstream tests;
benchmark extensions use unmodified consumer sources.

Distribution packaging requires separate validation of archive structure,
installed parser identity, native dependencies, and the target's libc baseline
and threaded parsing behavior. Installed XML tests and application benchmarks
must exercise the packaged interpreter, and the XML suites must pass without
exceptions. Check packaging and measure installed performance on each supported target.

## Safety and release criteria

The core forbids unsafe Rust. The storage and C boundary still need careful
ownership review, allocation-failure injection, callback reentry checks, Miri and
sanitizers. [Fuzzing](../fuzz/README.md) includes deterministic replay, mutation
campaigns and an isolated Expat oracle. Report the tested revision, scope and
duration of each run.

Before a production release, test the remaining semantic and resource differences
in each consumer, validate packaging on each target platform, run longer fuzz
campaigns, and obtain independent review. The performance goal is within roughly
20% of Expat on representative project XML through both C and CPython. Report
every workload and measure against inputs that were not used for tuning.
See [benchmarking](../benchmarks/HILLCLIMB.md) and [acceptance](../CONTRIBUTING.md#review-and-performance).

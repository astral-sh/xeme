# Conditional DTD and multibyte consumer validation

These results use one immutable release build containing the conditional-DTD and
multibyte-decoder changes. The shared library SHA-256 is
`912be874c54d597733a986c7cd3d0ea0bd36e0329e50b84c633908d59e1163f6`;
the static archive SHA-256 is
`b6031442902ac0b0adf85f93deac5e46db8582899d6ac31a2bdc30fa0a593cbf`.
The exact source manifest is [source.json](source.json).

| Check | Result |
| --- | --- |
| CPython 3.12.13, original consumer, shared linkage | 803 tests; 31 skipped; success |
| CPython 3.12.13, original consumer, static linkage | 803 tests; 31 skipped; success |
| CPython 3.12.13, upstream consumer fix, shared linkage | 803 tests; 31 skipped; success |
| CPython 3.12.13, upstream consumer fix, static linkage | 803 tests; 31 skipped; success |
| Native C integration, adversarial and allocation suites | All six shared/static runs passed |
| Native allocation-failure injection | 386 scenarios per linkage passed |
| Core and C-interface Rust tests | 128 passed |
| Core, C-interface and multibyte-fuzzer Clippy checks | Passed with warnings denied |

The CPython runs compile the actual `pyexpat` and `_elementtree` extension sources
and verify their loaded origin. They use the selected C memory suite without a
system-allocator adaptation. Tests and skipped counts are retained in full.

## Consumer allocation-failure behavior

The ordinary test suite passes with both original and fixed CPython sources.
The separate fault probe forces child-parser creation to return null. Original
CPython 3.12.13 then crashes during its failure cleanup; with the upstream fix,
both linkages raise `MemoryError` and retain the parent's reference count of ten.
This is the previously reported [CPython issue 144984](https://github.com/python/cpython/issues/144984),
fixed by [CPython PR 144992](https://github.com/python/cpython/pull/144992).
The PBS integration includes that backport. The archive records the original and
compiled source hashes and the patch hash separately.

## Canonical multibyte comparison

An independent ctypes probe compared the release library with Expat 2.8.4 in 688
case/chunk-size/deferral combinations. Status, error code and normalized callback
content matched in all 688. Raw byte indices and byte counts also matched. Twenty
combinations involving a UTF-8 BOM and a custom encoding declaration differed in
column numbers; all observations are retained in
`canonical-multibyte-results.json.gz`, including those differences. The exact
probe is preserved as `canonical-multibyte-probe.py.gz`.

The fixtures cover real non-ASCII conversion in names, attributes, namespace
URIs, entities, text, CDATA, processing instructions and comments, plus truncated
sequences and CRLF accounting. Noncanonical multibyte aliases for ASCII remain
explicitly unsupported; these results do not claim full Expat encoding
compatibility. The complete upstream API matrix is reported separately and
retains its failures.

## Evidence

[summary.json](summary.json) contains the compact results and provenance.
[manifest.json](manifest.json) hashes the published files.
`consumer-evidence.tar.gz` contains the original build/test/probe logs, summaries,
and compiled CPython source files; [archive-members.json](archive-members.json)
hashes every archive member. It contains no compiled libraries or executables.
The original archived summary was produced before fuzzing finished and retains
that historical pending pointer; the current summary links the completed
[sanitizer evidence](../../../../fuzz/results/2026-09-10/converter-and-family/README.md).

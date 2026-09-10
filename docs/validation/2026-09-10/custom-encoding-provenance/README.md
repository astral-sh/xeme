# Custom-encoding lexical provenance

Custom converters can now return valid ASCII characters without turning encoded
character data into raw XML syntax. An encoded sequence that produces `<` stays
character data; a raw `<` starts markup. Sparse records preserve original bytes
through incremental input, DTD composition and external entity-value continuations.
Semantic callbacks and lookups use decoded strings. End tags compare original
encoded name spellings, while duplicate attributes compare decoded names.

The frozen shared library is
`15e04ac1b107f7e2d388a0bad221ac58181612e281a3b410547c27c0a0f151fc`,
built on `21b76d1`. [The report](report.json) records hashes, results, limitations
and integration requirements. [Evidence](evidence.tar.gz) contains verified frozen
runtime sources, the patch, unchanged adapted upstream sources, scripts, callback
traces, native logs, independent review and performance samples. [The file
manifest](files.json) and [checksums](SHA256SUMS) identify every artifact. Later
edits only correct Rustdoc and add documentation and native tests.

## Compatibility

All **4,740 upstream API configurations complete: 3,823 pass and 917 fail**.
Compared with the consumed-byte accounting checkpoint, exactly 48 configurations
improve: the unknown-encoding success, long-name, protected-callback and bad-doctype
tests each gain 12 passes. No other result changes. Test assertions, allocation
retry ceilings and adapted-source hashes remain unchanged. The separately
committed ordinary version/comment/PI correction is included in this candidate.

The following grids compare the same frozen library with pinned Expat 2.8.4.
Adjacent text/default fragments are coalesced by the probe; this does not establish
exact callback fragmentation compatibility.

| Probe | Configurations | Acceptance differences | Successful callback differences |
| --- | ---: | ---: | ---: |
| Core lexical contexts, whole/1/7-byte feeds | 9,300 | 0 | 0 |
| Expanded contexts, namespaces and three lead bytes | 54,096 | 0 | 0 |
| Declaration, comment, PI and DTD callback payloads | 28,812 | 0 | 0 |
| External entities with a default handler | 7,644 | 0 | 5,112 |
| External entities without a default handler | 7,644 | 0 | 0 |
| PUBLIC identifiers and original map-byte classes | 7,154 | 0 | 0 |
| Alternate encoded name spellings | 64 | 0 | 0 |

The external default-handler differences concern declaration prefixes and closing
delimiters. A separate 117-case replay using ordinary ASCII spellings produces
identical baseline/candidate traces; 48 successful cases differ from Expat's
default callbacks in both versions. All malformed-input callback, diagnostic and
position differences remain in the archived reports. The table does not imply
zero total differences or complete Expat compatibility.

Converters remain limited to valid XML characters in the Basic Multilingual
Plane. Required raw ASCII map entries must retain their prescribed meanings.
Existing external value-child encoding-declaration restrictions, absolute resource
ceilings and unsupported ABI modes remain documented in the C interface README.

## Security and ownership

All **240 core/FFI tests**, formatting and strict affected-package Clippy pass.
Regressions exercise raw name identity, decoded duplicate attributes, namespace
aliases, typed attribute whitespace, public identifier byte classes, real Unicode
characters equal to scanner representatives, allocation failure, converter
reentry and encoding-instance release. Provenance records use the selected parser
allocator. Both metadata and text reservations succeed before logical mutation;
source compaction moves their offsets together. Ordinary UTF-8 input allocates
no provenance records. Existing resource budgets remain active.

All six shared/static native integration, adversarial and allocation suites pass;
each linkage includes 358 injected allocation failures. The public integration
suite also passes against pinned Expat and now checks ASCII aliases, original-byte
end-tag matching, decoded duplicate attributes, one-byte buffer input and release.
The C consumers use address/undefined sanitizers; the release Rust library is
uninstrumented and leak sanitizer is disabled.

A separate agent reviewed sparse metadata, allocation failure atomicity, raw-name
identity and PUBLIC classification and found no remaining blocker. This was a
bounded source review, not independent execution of the differential grids.
Selected name rules must also reach PUBLIC map classification when the historical
Expat name-mode layer integrates.

## Performance screening

Five randomized pairs on each of six pinned project XML files compare this layer
with its immediate ordinary-payload baseline using the native callback driver.
Each process runs a warmup and seven measured parses on CPU 1. Median
candidate/control throughput ratios range from **0.940–0.994 at 4 KiB** and
**0.967–0.984 at 64 KiB**. Two generated entity/declaration fixtures range from
0.971–0.995. All observed callback hashes, element counts and text totals agree.

This is a compatibility cost of roughly 1–6% in the small shared-host screen,
with substantial individual scheduling outliers. It is not a benchmark against
Expat or actual project process timing. The root performance layers were absent
from this isolated baseline; the combined implementation needs fresh measurements.
The widened lexical views and token ownership paths warrant further profiling.

The 917 upstream failures and the remaining broader performance/readiness work
are not resolved by this checkpoint.

# XML denial-of-service protections

Xeme implements its own XML parser; it does not load Expat. The protections below
apply to the Rust parser, CLI, and Expat-compatible C interface. Their resource
policies differ; [compatibility](compatibility.md#streaming-resource-limits)
defines the limits for each interface.

## Entity expansion

Both exponential expansion (Billion Laughs) and quadratic expansion (repeating a
large, shallow entity) consume shared byte budgets. Protection does not depend
only on nesting depth. General entities, attribute values, reused DTD defaults,
parameter entities, and external-entity children charge shared work. Consumed
root input supplies relative credit; buffered trailing bytes and external input
cannot increase that credit.

- The Rust parser and CLI default to an absolute 8 MiB cumulative work budget,
  including entity expansion, and 32 entity levels.
- The C interface allows cumulative work up to
  `max(8 MiB, 100 × consumed root bytes)`. Its separate Expat-compatible entity
  check limits combined direct and indirect bytes to 100 times consumed root
  input once the combined count reaches 8 MiB. Before any root input is consumed,
  external parsers use Expat's fixed 22-byte baseline.
- Cycles, declaration counts, token sizes, and nesting have separate checks.
  External children share their root's budgets. Children retained after parent
  destruction or reset keep the original document's work counters; a reset root
  starts new work counters. Checked counters reject overflow.

Budget violations during parsing are terminal errors: `ErrorKind::LimitExceeded`
in Rust and `XML_ERROR_AMPLIFICATION_LIMIT_BREACH` through the C interface.
Allocation limits can also reject input with an out-of-memory error. Applications
must stop on a parse failure. Lowering the relative activation threshold can
reject smaller expansions; increasing limits permits more work.

## Large unfinished tokens

[CVE-2023-52425](https://github.com/libexpat/libexpat/blob/R_2_6_0/expat/Changes)
concerned repeatedly reparsing growing tokens across small input calls in Expat
before 2.6.0. Xeme retains scanner progress across chunks and enables reparse
deferral by default. Token scanning also has a 16 MiB default ceiling. Final input
flushes pending parsing; token-limit and decoding errors are not postponed merely
because a token is deferred.

`set_reparse_deferral_enabled` and `XML_SetReparseDeferralEnabled` control deferral.
Keep it enabled for untrusted input. The Expat compatibility version reported by
Xeme describes its API target, rather than a linked copy of Expat.

## Compressed input

Xeme has no gzip, ZIP, zlib, bzip2, or LZMA decompressor and does not fetch HTTP
streams. The CLI reads file or standard-input bytes directly. Compressed files
must be decompressed by the caller before parsing.

An embedding application's decompressor needs its own output-size and work/time
limits, including for external entities. Feed decompressed data in bounded
chunks and stop decompressing when parsing fails. Fully decompressing a document
before calling Xeme spends memory and CPU before any parser limit can apply.
Parser entity-amplification checks measure XML input bytes, not compressed bytes,
so they cannot enforce a compression-ratio limit.

The Rust parser and CLI default to 256 MiB of cumulative input per source. The C
interface intentionally supports longer streams: its 256 MiB limit is **per input
call or buffer request**, and cumulative input is limited only by representable
positions. C consumers must enforce their own cumulative decompressed-byte and
request-time budgets. The C family's 512 MiB live-allocation ceiling covers
parser-owned backing allocations, not the caller's decompressor or retained
callback output.

These controls bound the described attack mechanisms. They do not make arbitrary
input sizes, caller callbacks, or caller-selected limits inexpensive. See the
[release criteria](compatibility.md#safety-and-release-criteria) for the broader
validation boundary.

## Regression coverage

The workspace test suite includes [entity attack inputs](../crates/xeme/tests/entity_attacks.rs),
[deterministic token-scanning work checks](../crates/xeme/src/large_token_tests.rs),
[streamed input limits](../crates/xeme/tests/input_limits.rs), and
[C input API attack checks](../crates/xeme_expat/src/tests.rs).
Attack fixtures have bounded theoretical expansion so regressions do not require
allocating gigabytes. Token complexity checks count scanning work rather than
depending on elapsed-time assertions.

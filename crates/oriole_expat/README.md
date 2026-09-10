# Oriole's Expat interface

This crate exports the ordinary `char` Expat C ABI as a shared library and static
archive. [The public header](../../include/expat.h) describes that ABI; its Expat
version macros and `XML_ExpatVersionInfo()` identify the targeted API revision,
while `XML_ExpatVersion()` reports Oriole's own identity. Reporting an API target
does not claim every optional capability is supported. The header retains the
upstream Expat authors' MIT notice.

## Ownership and callbacks

Each parser and its external-entity family require serialized access, as with
Expat's synchronous external DTD processing. Event strings and attribute
arrays remain valid only for their callback. No Rust reference to the parser crosses
a callback: handlers receive owned event data, and handler changes take effect for
subsequent events.

Callbacks can suspend, abort, change handlers, or free the parser. A callback's
`XML_ParserFree` request is deferred until the outer parse returns; that parse returns
`XML_STATUS_ERROR`, and the handle is then invalid. Recursive parsing, buffer
requests, and reset on the active parser fail with `XML_ERROR_UNEXPECTED_STATE`.
Recursive `XML_DefaultCurrent` calls are also rejected. A different parser can be
used from a callback. Rust panics in allocating entry points are caught before the C
boundary. Allocation failures on the C path return errors without allocating an
error message.

Content-model callbacks receive one C allocation containing the entire model and
its names. The consumer owns that allocation and releases it with
`XML_FreeContentModel`, including after freeing the parser.

## Current boundaries

The exported API includes streaming/buffer input, parser reset, suspension,
namespace processing, DTD declaration callbacks, internal and external entities,
foreign DTDs, default handlers, location/error access, and the C memory helpers.
External content comes exclusively from caller-provided callbacks. Oriole never
fetches external resources itself. Root reset disconnects children of the previous
document so they cannot modify declarations in the new document.

Complete `XML_Memory_Handling_Suite` values are supported. Every parser-owned
allocation, temporary callback buffer, child parser, and content model carries its
allocator. Incomplete suites are rejected. Allocation callbacks must implement the
C memory-allocation contract and must not reenter parser APIs; entry points reject
reentry before accessing a parser. Ordinary event callbacks remain reentrant for
the documented operations above.

Namespace separators must be ASCII bytes. The C constructors reject bytes
`0x80` through `0xff`, which cannot be represented as a single UTF-8 byte by the
Rust parser. ASCII separators retain their exact byte value; `\0` is supported
without namespace triplets.

The following Expat modes are explicitly unsupported:

- Parameter entities inside declarations and conditional `INCLUDE`/`IGNORE`
  sections are rejected. Parameter references between declarations are supported.
- Multibyte custom encoding conversion callbacks: single-byte custom maps work,
  including release callbacks on reset/free and on rejected maps. Maps requiring
  multibyte conversion report `XML_ERROR_UNKNOWN_ENCODING`.
- Caller-supplied hash salts and relative entity-amplification tuning: those setters
  return false. Oriole instead applies absolute input, token, nesting, attribute,
  and entity-expansion limits.
- Input context buffers: `XML_GetInputContext` returns null, and the feature table
  advertises `XML_CONTEXT_BYTES=0`.
- Wide-character and `XML_LARGE_SIZE` builds: the header rejects these configurations.

Reparse deferral is configurable, with progressive scanning in both modes. The
live-allocation tracker supports Expat's maximum-amplification and activation
threshold controls, including child allocations. Defaults are 100 times the root's
input size, activated at 64 MiB of live allocation. Allocation headers retain their
tracker through reallocation and through destruction of their original parser.
A separate 512 MiB ceiling on live backing allocations remains active even when
relative amplification checks are disabled.

A parser family shares a 256 MiB raw-input budget and an 8 MiB entity-expansion
budget. The C interface also caps aggregate event payloads at 64 MiB, child
creation at 1,024 parsers, and external-child ancestry at 32 levels.

These remaining boundaries prevent claiming complete Expat compatibility.
Unmodified CPython can use its standard custom allocator suite; the actual consumer
tests and their remaining failures are recorded separately from allocation-failure
tests. See [the release gates](../../docs/compatibility.md) for the integration plan.

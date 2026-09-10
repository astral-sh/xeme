# Oriole's Expat interface

This crate exports the ordinary `char` Expat C ABI as a shared library and static
archive. [The public header](../../include/expat.h) describes that ABI; its Expat
version macros and `XML_ExpatVersionInfo()` identify the targeted API revision,
while `XML_ExpatVersion()` reports Oriole's own identity. Reporting an API target
does not claim every optional capability is supported. The header retains the upstream Expat authors' MIT notice.

## Ownership and callbacks

Each parser requires serialized access, as with Expat. Event strings and attribute
arrays remain valid only for their callback. No Rust reference to the parser crosses
a callback: handlers receive owned event data, and handler changes take effect for
subsequent events.

Callbacks can suspend, abort, change handlers, or free the parser. A callback's
`XML_ParserFree` request is deferred until the outer parse returns; that parse returns
`XML_STATUS_ERROR`, and the handle is then invalid. Recursive parsing, buffer
requests, and reset on the active parser fail with `XML_ERROR_UNEXPECTED_STATE`.
Recursive `XML_DefaultCurrent` calls are also rejected. A different parser can be
used from a callback. Rust panics in allocating entry points are caught before the C
boundary; allocator exhaustion can still abort the process, as in ordinary Rust.

Content-model callbacks receive one C allocation containing the entire model and
its names. The consumer owns that allocation and releases it with
`XML_FreeContentModel`, including after freeing the parser.

## Current boundaries

The exported API includes streaming/buffer input, parser reset, suspension,
namespace processing, DTD declaration callbacks, internal entities, default
handlers, location/error access, and the standard C memory helper functions.

The following Expat modes are explicitly unsupported:

- Non-null `XML_Memory_Handling_Suite`: `XML_ParserCreate_MM` returns null. The
  contract requires **every** internal allocation to use the supplied suite; routing
  only the parser handle through it would violate that contract. A null suite works.
- External DTDs and parameter-entity expansion: external DTD child construction
  succeeds, but parsing that child reports an error. Enabling parameter expansion
  returns false. External general entities work through caller-provided callbacks
  and child parsers; Oriole never fetches external resources itself.
- Multibyte custom encoding conversion callbacks: single-byte custom maps work,
  including release callbacks on reset/free and on rejected maps. Maps requiring
  multibyte conversion report `XML_ERROR_UNKNOWN_ENCODING`.
- Caller-supplied hash salts and Expat's relative
  amplification/allocator tuning knobs: setters return false. Oriole instead applies
  its core's fixed absolute input, token, nesting, attribute, and entity limits.
  Reparse deferral is configurable, with progressive scanning in both modes.
  A parser family shares a 256 MiB raw-input budget and an 8 MiB entity-expansion
  budget. The C interface also caps aggregate event payloads at 64 MiB, child
  creation at 1,024 parsers, and external-child ancestry at 32 levels.
- Input context buffers: `XML_GetInputContext` returns null, and the feature table
  advertises `XML_CONTEXT_BYTES=0`.
- Wide-character and `XML_LARGE_SIZE` builds: the header rejects these configurations.

These boundaries prevent claiming a drop-in CPython replacement. In particular,
CPython uses a custom memory suite by default. Consumer tests that explicitly switch
to the system allocator measure callback compatibility, not allocator compatibility.
See [the release gates](../../CONTRIBUTING.md#acceptance) for the complete integration plan.

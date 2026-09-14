# Xeme's Expat interface

This crate exports the ordinary `char` Expat C ABI as a shared library and static
archive. [The public header](../../include/expat.h) describes that ABI; its Expat
version macros, `XML_ExpatVersionInfo()`, and the `xeme_compat_2.8.4` string from
`XML_ExpatVersion()` identify the same targeted API revision. The separate
`XEME_VERSION` header macro and Cargo package version identify the Xeme
implementation, currently `0.0.1`. Consumers that require Expat's literal version
string still distinguish Xeme. The header retains the upstream Expat authors'
MIT notice.

## Build the C libraries

From the workspace root, build the shared library and static archive with:

```sh
cargo rustc --release --locked -p xeme_expat --lib --crate-type cdylib,staticlib
```

The Cargo-level crate-type override lets the release profile's ThinLTO setting
apply to the C artifacts. The manifest also retains `rlib` for Rust tests and
consumers; asking Cargo to emit all three types in one build suppresses that LTO
step. Put `--crate-type` before any `--` so Cargo can prepare the dependencies for
LTO. The command produces `libxeme_expat.so` (or `.dylib` on macOS) and
`libxeme_expat.a` in `target/release` on Unix. An ordinary Cargo build into the
same directory can replace those artifacts, so keep the selected C build outputs
for consumer validation and packaging.

The [historical PGO guide](../../tools/pgo/) records the optional profile-guided
build procedure; PGO is no longer an optimization workstream.

## Ownership and callbacks

Each parser and its external-entity family require serialized access, as with
Expat's synchronous external DTD processing. Parent reset and destruction must
also be serialized with operations on its children. Event strings and attribute
arrays remain valid only for their callback. No Rust reference to the parser crosses
a callback: handlers receive owned event data, and handler changes take effect for
subsequent events.

Callbacks can suspend, abort, or change handlers. As in Expat 2.8.4, recursive
parsing, buffer requests, reset, and resume on the active parser fail without
changing its error state. `XML_ParserFree` during a callback is ignored; the caller
must free the handle after the outer operation returns. Recursive
`XML_DefaultCurrent` calls are rejected with `XML_ERROR_UNEXPECTED_STATE`. A different parser can be
used from a callback. Rust panics in allocating entry points are caught before the C
boundary. Allocation failures on the C path return errors without allocating an
error message.

A failed API request, such as requesting a negative buffer size or resuming an
unsuspended parser, updates the reported error without invalidating pending input.
A later valid parse or resume can proceed. XML syntax errors and failures while
processing input remain terminal until reset.

Content-model callbacks receive one C allocation containing the entire model and
its names. The consumer owns that allocation and releases it with
`XML_FreeContentModel`, including after freeing the parser.

## Current boundaries

The exported API includes streaming/buffer input, parser reset, suspension,
namespace processing, DTD declaration callbacks, internal and external entities,
foreign DTDs, default handlers, location/error access, and the C memory helpers.
External content comes exclusively from caller-provided callbacks. Xeme never
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
Rust parser. ASCII separators retain their exact byte value; `\0` concatenates
the URI and local name and ignores namespace triplet mode, as in Expat.
Choose a separator outside the URI character set, such as `|`, to keep expanded
names unambiguous. Following Expat, URI characters such as `:` are also supported
as legacy separators; collisions are rejected for non-URI separators, including
when the URI introduces that character through an XML character reference.

Custom encodings support single-byte maps and conversion callbacks for two- to
four-byte sequences. Each completed sequence is converted once, with original
byte widths retained for positions. Reset, free, and rejected maps release their
encoding instance once; external children acquire their own converter instance.
Converted values may be any valid XML character in the Basic Multilingual Plane,
including ASCII. Converted ASCII remains distinct from raw syntax: an encoded
sequence producing `<` is character data, while a raw `<` starts markup. Names,
references, declaration keywords, whitespace, and public identifiers retain their
original lexical roles. End tags compare original encoded name spellings;
callbacks and semantic lookups receive decoded strings. Sparse provenance uses
the selected parser allocator and existing memory and work budgets; ordinary
UTF-8 input does not allocate provenance records.

External DTD default-handler prefixes and some malformed-input errors, callback
prefixes, and positions still differ. Invalid maps, supplementary converted
characters, and forbidden XML characters remain errors; the external value-child
declaration restrictions below also apply.

The C interface uses XML 1.0 Fourth Edition name rules to match the pinned
Expat 2.8.4 reference, including in DTDs, references, and custom-encoding byte
classification. Resets and external children retain those rules. The Rust
interface defaults to Fifth Edition and can select either edition through
`Config::name_rules`.

Internal parameter entities can supply complete lexical tokens and grammar
delimiters inside declarations in external DTDs and parameter entities.
Replacement frames preserve name and quote boundaries, attribute whitespace,
and entity-value provenance. A replacement may close the containing declaration
or conditional header and leave further declaration grammar to resume in the
parent. References between declarations must contain complete declarations;
ignored conditional sections must close within their source.

External references between declaration tokens and in conditional headers load
separate DTDs. A child may supply declarations used by the remaining grammar;
it does not supply literal grammar or keyword text. Completed attributes and
declarations are reported before the next external callback, and a child cannot
replace its parent's reserved entity name. Skipped children stop later entity
and attribute declarations unless the document is standalone; initialized empty
children retain normal processing.

Nested `INCLUDE`/`IGNORE` sections and internal parameter entities selecting
conditional keywords or expanding entity values are supported in external DTDs
and parameter entities. External references inside entity values create separate
value children and resume the pending declaration with their output. Children
validate complete lexical tokens before storing value text and preserve quote
and reference boundaries. These paths share the family's expansion and depth
limits. Conditional nesting uses the element-depth ceiling; a whole ignored
section uses the token-byte ceiling. Value children distinguish unread children
from initialized empty or failed children, including partial output before a
storage error.

The following Expat modes are explicitly unsupported:

- A value child's encoding declaration must be first, apart from its byte-order
  mark. Expat also permits the first declaration after value content, including
  whitespace or a comment; Xeme cannot switch encoding after that prefix has
  been decoded. A leading custom-encoding declaration may request its encoding
  handler before delivering the XML-declaration callback; Expat reverses this
  callback order in its value processor.
- Wide-character, `XML_LARGE_SIZE` and `XML_ATTR_INFO` builds: the header rejects these configurations.

`XML_SetHashSalt` and `XML_SetHashSalt16Bytes` mix the caller salt into randomized
hashing; a predictable salt does not replace the secret random keys. They update
the root parser before parsing or after completion, including when called through
a child. They reject roots that are parsing, suspended, or executing a callback,
and children whose parent was freed or reset. Allocation failure preserves the old
salt and all tables. Reset preserves the configured salt. Newly created children
inherit it; existing children retain their independently owned hash states, so
changing the root never invalidates a child's populated tables.

`XML_GetInputContext` exposes original encoded bytes while parsing is active,
including entity-reference spellings and UTF-16 input. The buffer retains at least
1,024 bytes before pending input and remains valid for the requesting callback.
The getter returns null outside parsing and after reset.

Entity amplification supports Expat's maximum-factor and activation-threshold
controls. Root and child parsers share consumed input and replacement-byte counts;
unparsed trailing input does not increase the denominator. Defaults are 100 times
the consumed root input, activated at 8 MiB of combined direct and indirect bytes.
An external parser used before any root input follows Expat's 22-byte baseline.
These controls leave Xeme's input, token, nesting, attribute, and fixed work
policy active; the C work allowance grows with consumed root input.

The C constructors allow up to 100,000 declared entities and 100,000 levels of
internal entity expansion. Reset preserves these limits and external children
inherit them. The safe Rust API retains its defaults of 10,000 declarations and
32 entity levels. Iterative expansion and active-name indexes avoid a matching
host call stack or repeated scans of the active chain.

Reparse deferral is configurable, with progressive scanning in both modes. The
live-allocation tracker supports Expat's maximum-amplification and activation
threshold controls, including child allocations. Application-owned blocks requested
through `XML_MemMalloc`/`XML_MemRealloc` use the selected allocator but are exempt
from parser amplification accounting, including when requested inside a callback.
Defaults are 100 times the root's input size, activated at 64 MiB of live
allocation. Allocation headers retain their tracker through reallocation and through destruction of their original parser.
A separate 512 MiB ceiling on parser-owned live backing allocations remains active even when
relative amplification checks are disabled.

Input calls and buffer requests are limited to 256 MiB. Cumulative source input
is bounded by representable positions; work and event-payload allowances grow
with consumed root input. See [streaming resource limits](../../docs/compatibility.md#streaming-resource-limits)
for the limits and accounting rules. A parser family permits at most 1,024 child
construction attempts, including failures, and 32 levels of external-child ancestry.

Xeme does not decompress input. Consumers supplying decompressed streams must
bound cumulative decompressed bytes and decompressor work themselves; the per-call
input limit is not a document-size ceiling. See
[XML denial-of-service protections](../../docs/security.md).

CPython can use its standard custom allocator suite. See the
[compatibility guide](../../docs/compatibility.md) for remaining consumer
differences.

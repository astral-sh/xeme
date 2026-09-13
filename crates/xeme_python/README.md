# Xeme for Python

Xeme is an incremental XML parser for Python 3.10 and later.

## Install from a checkout

With Rust 1.96 or later installed, run these commands at the repository root:

```console
uv venv
uv pip install .
```

Alternatively, run `python -m pip install .` in an activated virtual environment.
The package is not yet published to PyPI. Building from source uses
[maturin](https://www.maturin.rs/); installing a built wheel does not need Rust.

## Parse XML

```python
from xeme import Parser

parser = Parser()
parser.feed(b'<message priority="high">Hello</message>', final=True)
for event in parser.read_events():
    print(event.kind, event.data)
# start ('message', {'priority': 'high'})
# text Hello
# end message
```

Feed `bytes` in chunks, fully consuming the `read_events()` iterator after each
chunk. Set `final=True` on the last chunk, which may be empty. Feeding before
consuming all events raises `RuntimeError`; feeding after final input raises
`ValueError`. Create a new parser for each document.

Events can be retained after subsequent input. Text may be split across events;
concatenate adjacent text events if your application needs a single string.

## Events and positions

Each event has read-only `kind`, `data`, and `position` properties. Start-element
attributes are an ordinary Python dictionary, independent of the parser.

| Kind | Data |
| --- | --- |
| `start` | `(name, attributes)` |
| `end` | Element name |
| `text`, `comment` | Text |
| `pi` | `(target, data)` |
| `start_ns` | `(prefix, uri)`; absent values use `None` |
| `end_ns` | Prefix, or `None` for the default namespace |
| `start_cdata`, `end_cdata` | `None` |
| `xml_decl` | `(version, encoding, standalone)`; omitted fields use `None` |
| `start_doctype` | `(name, system_id, public_id, has_internal_subset)` |
| `end_doctype` | `None` |

`event.position` has `line` (one-based), `column` (zero-based), `byte_index`
(zero-based input offset), and `byte_count` (input span length).

## Configuration and errors

```python
from xeme import Limits, ParseError, Parser

parser = Parser(
    namespace_separator="}",
    limits=Limits(max_depth=64, max_total_bytes=8 * 1024 * 1024),
)
try:
    parser.feed(b'<root xmlns="urn:example"/>', final=True)
    events = list(parser.read_events())
except ParseError as error:
    print(error.kind, error.line, error.column, error.byte_index)
```

Namespace processing is opt-in. A separator must be exactly one Unicode
character. Expanded names take the form `uri + separator + local_name`.
`namespace_triplets=True` appends `separator + prefix` for prefixed names and
requires a separator. With `namespace_separator="\0"`, URI and local name are
concatenated directly and no triplet suffix is added. Namespace declarations
produce `start_ns` and `end_ns` events instead of appearing as attributes.

`encoding="..."` optionally requests an input encoding. Built-in encodings include
UTF-8, UTF-16, UTF-16LE, UTF-16BE, ISO-8859-1, and US-ASCII (also named ASCII).
Input must be bytes so that encoding detection and byte positions refer to the
original document.

`Limits` accepts keyword arguments and has read-only properties:

| Limit | Default |
| --- | --- |
| `max_depth` | 256 |
| `max_token_bytes` | 16 MiB |
| `max_total_bytes` | 256 MiB |
| `max_entity_expansion_bytes` | 8 MiB |
| `max_entity_depth` | 32 |
| `max_attributes` | 10,000 |
| `max_entities` | 10,000 |

`ParseError` reports malformed XML, unsupported encodings, and exceeded limits.
Its `kind` is a name such as `TagMismatch`; `line`, `column`, and `byte_index`
identify the error location. It can arise during `feed()` or `read_events()`.
Parse errors are terminal: subsequent operations raise the stored error.
Allocation failures raise `MemoryError` and also leave the parser unusable.

## Scope

This API supports internal DTDs, internal entity expansion, and default
attributes. It rejects external DOCTYPE identifiers and external general entity
references, performs no filesystem or network access, and exposes no external
entity resolver or custom encoding callback. It checks XML syntax without
validating DTD content models.

The Python event API is separate from `xml.parsers.expat`'s callback API and is
not a drop-in replacement. The [C interface](../xeme_expat/) and
[CPython consumer harness](../../tools/cpython/) cover existing Expat consumers.

See [contributing](../../CONTRIBUTING.md#python-bindings) for build and test commands.

# Command-line usage

Xeme checks XML syntax and can print parser events. It is experimental and does
not validate documents against DTD content models. See the
[compatibility guide](compatibility.md) for remaining Expat differences.

## Installation

Build from a checkout with Rust 1.96 or later:

```console
cargo install --path crates/xeme_cli --bin xeme --locked
```

The executable uses jemalloc on supported Unix platforms and mimalloc on Windows.
Add `--no-default-features` to use the system allocator. Library users retain
control over their allocator.

## Check a document

```console
xeme message.xml
xeme --events message.xml
xeme --events --namespaces --chunk-size 4096 message.xml
```

Omit the file or pass `-` to read standard input. Invalid XML exits with a nonzero
status. Files whose names start with `-` can follow `--`.

| Option | Behavior |
| --- | --- |
| `--events` | Print parser events |
| `--namespaces` | Expand namespace names with `\|` as the separator |
| `--chunk-size BYTES` | Read at most this many bytes per chunk; defaults to 65,536, with a range of 1–16,777,216 |
| `--help` | Print command-line help |
| `--version` | Print the version |

The parser handles elements, attributes, namespaces, comments, processing
instructions, CDATA, character references, entities, and DTD attribute defaults.
It supports UTF-8, UTF-16, ASCII, and ISO-8859-1 input. The CLI reads the selected
file or standard input and does not fetch external resources.

Compressed input is not decompressed automatically. When piping a decompressor's
output into Xeme, bound its output and work separately; see
[XML denial-of-service protections](security.md#compressed-input).

The CLI uses the parser's default resource limits. Applications that need different
limits or external-entity resolution can use the [Rust library](library.md) or
[Expat interface](../crates/xeme_expat/).

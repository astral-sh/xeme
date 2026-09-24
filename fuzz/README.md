# Fuzzing

## Targets

`parse` checks arbitrary bytes with both a 64 KiB token limit and a small token
limit selected by the first input byte, exercising token errors independently of
the total-input limit.

`streaming` compares acceptance and owned events for whole-input and incremental
parsing. It coalesces adjacent character-data fragments, since callbacks can split
text differently depending on chunking. Its first byte selects chunk size and
namespace mode.

`ffi` drives valid C parser handles through arbitrary chunks, buffer input,
reset, suspension, callback changes, callback-time deletion, and rejected
reentry. It retains and frees content models and injects failure into a custom
allocator, checking that every successful allocation is released. It does not
pass invalid pointers or deliberately use already-freed handles. The allocation
selector can fail any of the first 512 allocations. Test the decoder with
`cargo test --manifest-path fuzz/Cargo.toml --lib`.
Bits 1–3 of the second control byte select a built-in protocol encoding.
Named Unicode-signature seeds exercise detection with a conflicting protocol.

`ffi_family` creates external-parser families with a custom allocator, then
varies child input, allocation failures, buffer requests, parent reset, and
parent/child destruction order. Surviving children parse after their
parent is reset or freed, exercising parser lifetimes and external DTD merging.
It uses eight handle slots and no callback recursion.

`multibyte` installs custom two-, three-, and four-byte encodings,
then varies converter results, incomplete characters, chunking, buffer input,
callback suspension and abort, and rejected reentry. It checks that encodings are
released exactly once and allocations are balanced, including children parsed
after their parent is freed or reset. Named seeds cover valid non-ASCII conversions,
invalid maps and scalars, ASCII conversion rejection, and allocation failures.

`value_family` resolves external parameter references inside entity values through
C callbacks. It allows eight parser slots and four callback levels; each slot
owns its custom-encoding callback state. It varies unread, initialized empty,
partial, completed, ignored-error, and rejected children, then feeds retained
children after their parent is reset or
freed. Callback changes, rejected reentry, buffer input, non-ASCII conversions,
allocation faults, duplicate declarations, parameter-supplied quoted values, and
missing declaration grammar references exercise value continuations and raw
declaration callbacks. Every live handle is freed once; encoding releases and
custom allocations must balance. Error-returning operations use valid live handles.

Thirty-two control bytes precede the `p` and `q` payloads. Bytes 13–14 select the
split; bytes 17–23 select each child slot's lifecycle, byte 25 selects the DTD
template, and byte 28 selects the depth that receives callback actions. The
target permits at most 64 external requests, 4,096 feeds per parse, and eight
resume attempts per feed.

## Running

```console
cargo fuzz run parse -- -max_len=65536 -max_total_time=300
cargo fuzz run streaming -- -max_len=65536 -max_total_time=300
cargo fuzz run ffi fuzz/seeds/ffi -- -max_len=65536 -max_total_time=300
cargo fuzz run ffi_family fuzz/seeds/ffi_family -- -max_len=65536 -max_total_time=300
cargo fuzz run multibyte fuzz/seeds/multibyte -- -max_len=65536 -max_total_time=300
cargo fuzz run value_family fuzz/seeds/value_family -- -max_len=65536 -max_total_time=300
```

Use an instrumented nightly toolchain with cargo-fuzz. Reduce crashing inputs and
add regression tests before removing crash artifacts.
Record the source revision, command, toolchain, sanitizer, bounds, elapsed time,
and corpus hash for each campaign. Report fixed-input replays and mutations
separately: libFuzzer's final counter includes initialization and inputs skipped
by the target's filters.

## Expat semantic oracle

`expat_differential` compares parse success and successful element, attribute, and
character-data callbacks against Expat 2.8.5. It compares UTF-8 inputs up to 8,192
bytes, excluding NUL bytes and the substrings `<!DOCTYPE` and `<?xml`. UTF-16,
DTD/entity grammar, and encoding declarations are outside this target. Xeme
resource-limit errors (43) also skip comparison. Error codes, positions, and
partial callback prefixes on failure are not compared. Adjacent observed text
callbacks are coalesced. A leading control byte selects namespace mode and a chunk
size from 1 through 128.

Expat runs in a separate persistent process to prevent `XML_*` symbol interposition.
The oracle requires an Expat 2.8.5 version greeting. A dead process, invalid reply,
broken pipe, or timeout fails the run. Each exchange has a five-second oracle
parse alarm and a ten-second libFuzzer input timeout. Protocol errors and semantic
mismatches stop and reap the child; stdin EOF releases it on normal fuzzer exit.

Build a normal Expat shared library from revision
`4b3f0b06f39fb5529cead381694f8929901bc273`, then run:

```console
cc -std=c11 -O2 -Wall -Wextra -Werror -Iinclude fuzz/expat_oracle.c \
  /absolute/expat-build/libexpat.so -Wl,-rpath,/absolute/expat-build \
  -o /tmp/xeme-expat-oracle
XEME_EXPAT_ORACLE=/tmp/xeme-expat-oracle \
  cargo fuzz run expat_differential --sanitizer address fuzz/seeds/expat_differential -- \
  -max_len=8193 -timeout=10 -rss_limit_mb=1536 -max_total_time=300
```

The size bound includes the control byte. Record the reference revision,
oracle/library hashes, and compiler command alongside the campaign metadata.
Rust sanitizer instrumentation covers the fuzz target and Xeme; this command
builds the separate C oracle without sanitizer instrumentation. Execution totals
include filtered inputs and do not give an actual oracle-comparison count.

Retained seeds include a reduced CR-plus-quote acceptance bug and an overlapping
processing-instruction delimiter that caused a DTD slicing panic.

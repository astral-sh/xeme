# Fuzzing

The `parse` target checks arbitrary bytes against bounded parser limits. The
`streaming` target compares acceptance and owned events for whole-input and
incremental parsing; it coalesces adjacent character-data fragments because XML
callback fragmentation may vary by input chunking. Its first byte selects chunk
size and namespace mode.

The `ffi` target drives valid C parser handles through arbitrary chunks, buffer
input, reset, suspension, callback changes, callback-time deletion, and rejected
reentry. It retains and frees content models and injects failure into a custom
allocator, checking that every successful allocation is released. It does not
pass invalid pointers or deliberately use already-freed handles. The allocation
selector uses separate enable and ordinal bits, so all failure positions from 1
through 512 are reachable. Its exhaustive decoder check is
`cargo test --manifest-path fuzz/Cargo.toml --lib`.
Bits 1–3 of the second control byte select a built-in protocol encoding.
Named Unicode-signature seeds exercise detection with a conflicting protocol.

The `ffi_family` target creates bounded external-parser families with a custom
allocator, then varies child input, allocation failures, buffer requests, parent
reset, and parent/child destruction order. Surviving children parse after their
parent is reset or freed, covering the lifetime token and external DTD merge.
It uses eight handle slots and no callback recursion.

The `multibyte` target installs custom two-, three-, and four-byte encodings,
then varies converter results, incomplete characters, chunking, buffer input,
callback suspension and abort, and rejected reentry. It checks once-only encoding
release and custom allocator balance, including children parsed after their
parent is freed or reset. Named seeds cover valid non-ASCII conversions, invalid
maps and scalars, ASCII conversion rejection, and allocation failures.

The `value_family` target resolves external parameter references inside entity
values through actual C callbacks. Eight stack-owned slots and four callback
levels bound parser families; each slot owns its custom-encoding callback state.
It varies unread, initialized empty, partial, completed, ignored-error, and
rejected children, then feeds retained children after their parent is reset or
freed. Callback changes, rejected reentry, buffer input, non-ASCII conversions,
allocation faults, duplicate declarations, parameter-supplied quoted values, and
missing declaration grammar references exercise value
continuations and raw declaration callbacks. Every live handle is freed once;
encoding releases and custom allocations must balance. Error-returning operations
use valid live handles. No input is required to be well-formed XML.

Thirty-two control bytes precede the `p` and `q` payloads. Bytes 13–14 select the
split; bytes 17–23 select each child slot's lifecycle, byte 25 selects the DTD
template, and byte 28 selects the depth that receives callback actions. The
target permits at most 64 external requests, 4,096 feeds per parse, and eight
resume attempts per feed. Named seeds document meaningful successful and adversarial paths.

```console
cargo fuzz run parse -- -max_len=65536 -max_total_time=300
cargo fuzz run streaming -- -max_len=65536 -max_total_time=300
cargo fuzz run ffi fuzz/seeds/ffi -- -max_len=65536 -max_total_time=300
cargo fuzz run ffi_family fuzz/seeds/ffi_family -- -max_len=65536 -max_total_time=300
cargo fuzz run multibyte fuzz/seeds/multibyte -- -max_len=65536 -max_total_time=300
cargo fuzz run value_family fuzz/seeds/value_family -- -max_len=65536 -max_total_time=300
```

Use an instrumented nightly toolchain with cargo-fuzz. Keep discovered inputs,
reduce them, and add deterministic regressions before removing crash artifacts.
Record the source revision, command, toolchain, sanitizer, bounds, elapsed time,
and corpus hash for each campaign. Report fixed-input replay, initialization, and
mutation executions separately: the final libFuzzer counter includes initialization.
Generated executions can also return early at a target's scope filters. A short
smoke run establishes harness operation, not security or XML conformance.
Historical campaigns qualify only their recorded source; rebased code needs fresh
checks.

## Expat semantic oracle

`expat_differential` compares parse success and successful element, attribute, and
character-data callbacks against Expat 2.8.4. It compares UTF-8 inputs up to 8,192
bytes, excluding NUL bytes and the substrings `<!DOCTYPE` and `<?xml`. UTF-16,
DTD/entity grammar, and encoding declarations are outside this target. Xeme
resource-limit errors (43) also skip comparison. Error codes, positions, and
partial callback prefixes on failure are not compared. Adjacent observed text
callbacks are coalesced. A leading control byte selects namespace mode and a chunk
size from 1 through 128.

Expat runs in a separate persistent process to prevent `XML_*` symbol interposition.
The oracle requires an Expat 2.8.4 version greeting. A dead process, invalid reply,
broken pipe, or timeout fails the run; the harness does not silently restart and
discard the input. A five-second oracle parse alarm and ten-second libFuzzer input
timeout bound each exchange. Detected protocol errors and semantic mismatches
stop and reap the child; stdin EOF releases it on normal fuzzer exit.

Build a normal Expat shared library from revision
`12cf0b1f25f026a022fe728ad8f7e3d017285b80`, then run:

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

Retained seeds include the reduced CR-plus-quote acceptance bug and the overlapping
processing-instruction delimiter that caused a DTD slicing panic. Keep these fixed
regressions alongside subsequent mutation campaigns.

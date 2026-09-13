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
pass invalid pointers or deliberately use already-freed handles.

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
Record the revision, command, toolchain, elapsed time, executions, and corpus hash
for each campaign. A short smoke run establishes harness operation; it does not
establish security or XML conformance.

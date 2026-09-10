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

```console
cargo fuzz run parse -- -max_len=65536 -max_total_time=300
cargo fuzz run streaming -- -max_len=65536 -max_total_time=300
cargo fuzz run ffi fuzz/seeds/ffi -- -max_len=65536 -max_total_time=300
```

Use an instrumented nightly toolchain with cargo-fuzz. Keep discovered inputs,
reduce them, and add deterministic regressions before removing crash artifacts.
Record the revision, command, toolchain, elapsed time, executions, and corpus hash
for each campaign. A short smoke run establishes harness operation; it does not
establish security or XML conformance.

The committed FFI seed reproduces an overlapping processing-instruction delimiter
inside a DTD. The original campaign found a Rust slicing panic; the scanner now
looks for the closing delimiter after the opener, and parsing checks both
delimiters before slicing. The core regression exercises every chunk size.

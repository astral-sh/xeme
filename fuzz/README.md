# Fuzzing

The `parse` target checks arbitrary bytes against bounded parser limits. The
`streaming` target compares acceptance and owned events for whole-input and
incremental parsing; it coalesces adjacent character-data fragments because XML
callback fragmentation may vary by input chunking. Its first byte selects chunk
size and namespace mode.

```console
cargo fuzz run parse -- -max_len=65536 -max_total_time=300
cargo fuzz run streaming -- -max_len=65536 -max_total_time=300
```

Use an instrumented nightly toolchain with cargo-fuzz. Keep discovered inputs,
reduce them, and add deterministic regressions before removing crash artifacts.
Record the revision, command, toolchain, elapsed time, executions, and corpus hash
for each campaign. A short smoke run establishes harness operation; it does not
establish security or XML conformance.

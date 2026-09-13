# Callback-driven value-family fuzzer

The new target loads external DTDs and parameter entities through actual C callbacks. Sixty named seeds cover values, nested children, custom encodings, handler changes, allocator failures, and retained children after parent reset/free. Stack-owned slots and explicit operation bounds keep the harness itself bounded. Every case checks allocation and encoding-release balance.

Independent source review, strict Clippy, formatting, ASan compilation, and all 60 seed replays pass on the combined runtime from PR #44. The grammar seeds reproduce the missing-Default-handler panic on the earlier isolated runtime and pass its integrated correction. The handoff retains that negative experiment and exact source/binary hashes. Sustained campaign results follow separately.

CI runs this target under cargo-fuzz for 30 seconds alongside the existing parser, streaming, C API, family, and multibyte targets. A short smoke run does not establish exhaustive memory safety or compatibility.

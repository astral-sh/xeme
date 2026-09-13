# Copy retained input context in bulk

The C input-context retention path used allocator-api2's generic byte-by-byte `extend_from_slice`. We use the existing fallible storage copy helper, which reserves once and copies the initialized slice in bulk. Retention boundaries, indexing, allocator selection, accounting and callback guards retain their order. No new unsafe code is introduced.

The integrated library is `d3a8c3c35136ced74a8e5e53137c0ce1b7059160ab15ae57db32ef38a8fd1d5d` on parent `59d61acb2a5dcf6d31b97e6c8b3a376a9fdce06b`. All **74 FFI/memory Rust checks**, strict Clippy/formatting, **1,518 exact differential comparisons**, **32 context/suspension/handler-switch probes** and six shared/static native C suites pass, including **333 allocation failure scenarios per linkage**. C callers are ASan/UBSan instrumented; Rust is uninstrumented, leak sanitizer is disabled, and selected live allocation counts are checked.

The integrated paired six-project screen (4 KiB, namespaces off/on, five randomized groups, five measured parses after warmup) improves all medians by **2–13%** over the preceding runtime. All native output hashes/counts match. The isolated 4/64 KiB screen records 5–14% improvements, and Wayland instructions fall 43,009,988 → 37,881,488 (**11.9%**). Every integrated project remains slower than Expat. These shared-host screens are not a final full-application benchmark.

We also preserve a rejected callback-handler snapshot experiment. It removed eager pointer loads and reduced instructions by only 0.52%, with no consistent project speedup. That runtime patch is not included.

[evidence.tar.gz](evidence.tar.gz) contains exact source/build hashes, commands, raw samples, focused validation and scoped source review. ELF/static artifacts are excluded and identified by hash. [files.json](files.json) verifies every member. Archive SHA256: `fa713866722801404de08f0e3f70650dc727ca7c0865fa22220f5bf91ed9c35b`. This layer does not claim a new full API/CPython/PBS campaign.

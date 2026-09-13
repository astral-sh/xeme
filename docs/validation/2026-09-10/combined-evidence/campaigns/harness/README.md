# Callback-driven external value family fuzzing

Adds `value_family`, a bounded C API libFuzzer target, plus 60 named seeds. The
harness loads external DTD and value children through the actual external-entity
callback, varies child completion and lifetime, and checks allocator and encoding
release balance after all parser handles are freed.

Coverage includes custom two-/three-/four-byte encodings, arbitrary converter
results, callback reentry and stop requests, handler changes, ParseBuffer, parent
free/reset, duplicate declarations, missing references, and parameter-supplied
quoted values with grammar references before and after the value. It retains
children after their parents disappear and finalizes them using valid live
handles. Eight fixed slots, depth four, 64 requests, 4,096 feeds, eight resume
attempts and 64 KiB input bound each execution.

Apply `value-family.patch` and unpack `seeds.tar.gz` from the repository root.
The patch changes only fuzz/Cargo.toml, fuzz/README.md, and the new target. Seed
hashes and an independent source review are included. No runtime source changes
are part of this handoff.

Strict scoped Clippy, formatting, the ASan build, and all 60 seed replays pass
against the corrected combined runtime (root source manifest cc0f0088). The
new no-Default PE-quoted seed reproduced the already identified integration
panic in the previous runtime and passes after the correction. Longer final
campaigns and their exact source/binary/corpus evidence are being packaged
separately; seed replay alone is not a security claim.

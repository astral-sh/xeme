# Production C-library commands

The nine-file patch uses `cargo rustc --lib --crate-type cdylib,staticlib` for active C builds while keeping `rlib` in Cargo.toml. CI runner and uv commands are unchanged. No Rust implementation, header, dependency or resource-limit changes.

PGO preserves global Cargo option provenance and uses single verbosity without changing its strict warning gate. PBS now removes an empty encoded-flags variable so required PIC/unwind flags take effect; nonempty values still reject. CI adds a stable Rust/rust-docs PBS bundle smoke. The historical PGO human README qualifies its unsupported effective-ThinLTO claim; archived historical bytes stay unchanged.

Validation: 13 unit tests, Ruff/format/type checks, PBS recipe validation; fresh effective-ThinLTO PGO generate/replay 288+288 exact parses; actual PBS bundle with three fresh workspace compiler commands, PIC/unwind, native-library extraction and weak TLS hook. PGO used Ohm defaults disabled; PBS used default Ohm. Stable CI and full PBS distribution are not claimed.

Read `summary.json` for initial failures and limited attempts. They remain separately retained, including the double-verbosity warning rejection and PBS controller/environment/cache cases. The final PGO and PBS successful runs are distinct from earlier normal/ThinLTO correctness and timing studies.

`candidate.patch` is against PR112 base 1cb326c. Apply only these nine files; root-authored current README/report changes are outside this package. No commit or push by the author.

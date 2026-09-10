# Iterative attribute expansion

The parser expands attribute entities with an explicit borrowed frame stack and a salted active-name set. Parent lexical slices preserve custom-encoding provenance, and recycled output stays owned until expansion succeeds. Entity count, depth, expansion budgets, and amplification accounting are unchanged.

Validation: 288 Rust checks, the expanded allocation fail-point workload, strict Clippy, six native C ASan/UBSan suites, and 3,918 exact ordinary differential cases pass. All 4,740 upstream API outcomes remain 4,065 passed/675 failed. All 5,796 attribute oracle cases are identical to the preceding implementation. The 60,000-level small-stack test raises limits locally; default limits remain in place.

The oracle retains 252 existing NDATA error differences, 732 existing replacement-CRLF callback differences, and 2,460 position differences. The 132 custom-conversion conditions retain 30 existing replacement-CRLF differences. These are corrected or classified in separate layers, not waived here.

`report.json` records exact scope, hashes, and the independent source review. `files.json` inventories every file in `evidence.tar.gz`. Native C sanitizers do not instrument the release Rust library; leak sanitizer was disabled.

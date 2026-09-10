# Attribute replacement semantics

Physical CR/LF in an attribute becomes one space. CR and LF stored in an entity replacement become two spaces. Direct numeric or custom-converted line endings retain their literal values. Defaults use their recorded source origin, and their lexical metadata stays attached throughout normalization. NDATA entities in attributes report Expat error 15.

All 6,468 attribute conditions match Expat in acceptance, errors, attribute/default callbacks and child outcomes, correcting 1,200 conditions. All 372 custom-conversion conditions match, correcting 84 baseline differences. The additional 384 malformed/default-origin conditions preserve every baseline status and error; 12 accepted parameter defaults gain the same whitespace correction. Original inputs and outcomes are retained in the archive. The 2,580 existing position differences remain counted separately.

Validation: 291 Rust checks, strict Clippy, six native C ASan/UBSan suites, and 3,918 exact ordinary differential cases pass. All 4,740 upstream API outcomes remain 4,065 passed and 675 failed. No limits or accounting operations changed.

`report.json` records exact scope, hashes, and the independent source review. `files.json` inventories every file in `evidence.tar.gz`. Native C sanitizers do not instrument the release Rust library; leak sanitizer was disabled.

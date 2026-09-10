# Compose attribute replacement semantics

This checkpoint combines the [attribute semantic correction](../attribute-replacement-semantics/) with the full root stack, including DOCTYPE callbacks and iterative attribute expansion. Stored replacement CR and LF retain two whitespace characters; physical CRLF retains one. Lexical conversion metadata stays attached to default attributes, and NDATA references report the correct error.

The release build passes **292 Rust checks**, strict Clippy/formatting, **1,518 exact ordinary comparisons**, and six native C sanitizer suites with **336 allocation failure scenarios per linkage**. All **6,468 attribute conditions** match Expat acceptance, errors, attributes/defaults and child outcomes, correcting **1,200** conditions. All **372 custom-conversion conditions** match, correcting **84**. The **384 malformed/default-origin conditions** preserve every baseline status/error; **12** accepted defaults gain the same whitespace correction. Existing **2,580 position differences** remain counted separately.

All **4,740 upstream API outcomes** remain **4,065 passing / 675 failing**. No resource limits, accounting operations or allocator ownership rules change. C sanitizers cover callers; the Rust release library is uninstrumented and leak sanitizer is disabled. Selected live allocations are checked.

[report.json](report.json) records the frozen runtime/library hashes. [files.json](files.json) verifies [evidence.tar.gz](evidence.tar.gz), including commands, exact inputs and raw failures. Original semantic evidence remains separate; this checkpoint makes no project throughput claim.

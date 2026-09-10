# Preserve DOCTYPE callback delimiters

A start-doctype callback handles the closing `>` of a declaration without an internal subset. With an internal subset, the closing bracket and whitespace belong to the current start-handler state before external loading; the final `>` belongs to end-doctype handling. We retain that decision across handler changes, suspension and external callbacks, and reset it with the parser.

The integrated release library is `34b088c89641a1d1f00f9c42d2b4d8ae5de9f0115e70ade947b3599e121c99aa` on parent `af44fbca039b5b4ee1f01ddb100aa17b699f7f8a`. **280 Rust checks**, strict Clippy/formatting, **1,518 exact ordinary comparisons**, and six shared/static native suites pass, including **336 allocation failure scenarios per linkage**. C callers use ASan/UBSan; Rust is uninstrumented, leak sanitizer is disabled, and selected live allocations are checked.

All **4,104** focused handler/mutation/chunk/UTF-8/UTF-16 conditions match callback content/order and acceptance/errors. The **7,644** custom external-DTD conditions fix **5,328** observations, including every **5,112** successful callback-content difference. All **2,316** remaining failed-input observations are byte-for-byte unchanged. The unchanged full **4,740** API matrix stays **4,065 passed / 675 failed**, with no changed outcomes.

Exact fragment boundaries remain a failed gate: **360** focused conditions aggregate existing closing-whitespace chunks differently. Their callback content/order matches; no extra delimiter bytes remain. The isolated baseline, intermediate formatting checkpoint, full raw observations and failed strict gate are retained. This layer does not claim new project throughput, CPython or PBS measurements.

[evidence.tar.gz](evidence.tar.gz) contains frozen source/build hashes, raw results, scoped independent source review and integration checks. ELF/static binaries are excluded and identified by hash. [files.json](files.json) verifies every archive member. Archive SHA256: `d1d1009faace0851cc491dd0d5ebac161b68158c200596eed1f67be31dfba37d`.

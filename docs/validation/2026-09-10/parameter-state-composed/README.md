# Compose parameter-family state with DOCTYPE callbacks

This checkpoint combines the separately preserved [shared-state implementation](../shared-parameter-state/) with the [DOCTYPE callback correction](../doctype-default/). The only merge conflict appended independent C regression tests; both functions remain intact.

The frozen release library is `cf5562476c0f8e17c3388e17384274da1095c245363f20edf2ab5d6a59d7b7f7` at runtime commit `7c0d8b35c079c00da3012628ca56d7b35d2c5cd4`. All **284 Rust checks**, strict Clippy/formatting, **1,518 exact ordinary comparisons**, **six exact family cases**, and **4,104 exact callback-content/order DOCTYPE conditions** pass. The **1,944** missing-parameter cases retain zero acceptance differences and the existing **24** child-execution / **162** event differences. Full API results remain **4,065/675**, with every one of the **4,740** outcomes unchanged from the parent.

Six shared/static native C suites pass, including **336 allocation failure scenarios per linkage**. C callers use ASan/UBSan; Rust is uninstrumented, leak sanitizer is disabled, and selected live allocations are checked. Original source review, exact integrated hashes, commands, raw results and failed compatibility observations are retained. No new project performance or PBS claim is made by this checkpoint.

[evidence.tar.gz](evidence.tar.gz) excludes ELF/static artifacts and identifies them by hash. [files.json](files.json) verifies every member. Archive SHA256: `8ddfab6be6cf61aa152881f4a3dfdb1db936766bca9eeb408698d78f935dd824`.

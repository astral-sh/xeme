# Compose iterative attributes with corrected DOCTYPE callbacks

The root integration preserves the original [iterative attribute implementation and review](../iterative-attributes/) and composes it with corrected DOCTYPE callback routing. Entity limits and expansion accounting remain unchanged.

The frozen release build passes **289 Rust checks**, strict Clippy and formatting, **1,518 exact ordinary comparisons**, and six shared/static C sanitizer suites (**336 selected-allocation failure scenarios per linkage**). All **5,796 attribute observations** and **132 custom-conversion conditions** remain identical to the preceding root implementation, including existing diagnostic and replacement-CRLF differences. The next semantic layer addresses those attribute differences.

All **4,740 upstream API outcomes** remain **4,065 passing / 675 failing**. The original 60,000-level small-stack and bounded wide-entity tests remain in the Rust suite; default resource limits are unchanged. C sanitizers do not instrument the Rust release library; leak sanitizer is disabled and selected live allocations are checked.

[report.json](report.json) records the exact source and library hashes. [files.json](files.json) verifies all members of [evidence.tar.gz](evidence.tar.gz), including commands and raw observations. This checkpoint makes no project throughput claim.

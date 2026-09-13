# Composed shared DTD definitions

Parameter children publish committed declarations immediately, including declarations preceding nonfinal input or an accepted child error. General children retain independent snapshots. Table ownership stays inside safe scopes; no table reference reaches a callback. Shared family metadata limits and salt remain separate from parser-local state.

This exact runtime passes 319 core/adapter Rust checks and strict lint/format gates. All 4,740 upstream API observations remain 4,077 passing and 663 failing. Existing ordinary, attribute, malformed, custom-encoding, DOCTYPE and child-context gates retain their results. Six native C ASan/UBSan executions pass, with 336 selected-allocation scenarios per linkage; Rust release is uninstrumented and leak sanitizer is disabled.

The 432-case legal callback publication oracle improves from 264 full differences to nine. Parent and child outcomes match Expat. Remaining nine rows retain the standalone error24 callback-prefix timing gap; raw rows are preserved. The original candidate package under `../shared-dtd-definitions/` records selected allocations and paired timings, including the original tiny-document slowdown. This checkpoint precedes the separate no-owner fast-path correction.

`source.json` and `report.json` identify the exact runtime and results. `differential-correction.json` retains an initial affinity-script mistake that changed the chunk grid; the intended grid was rerun successfully, with both runs preserved. No runtime source changed for that harness correction.

# Shared DTD integration with ordinary-document fast path

Parameter children share committed declarations, including those preceding accepted nonfinal input or a child error. General children retain independent snapshots. An ordinary parser without a shared DTD owner or active DOCTYPE now calls the existing event body directly; DTD initialization and safe publication scopes keep their original behavior.

This final layer passes 319 core/adapter Rust checks and strict Clippy/formatting. All 4,740 original API observations remain 4,077 passing and 663 failing, with zero changes from the original shared-DTD candidate. The 432 publication cases preserve the reduction from 264 to nine full differences, with parent/child outcomes matching Expat. Remaining nine are recorded standalone error24 callback-prefix timing differences. Existing ordinary, attribute, malformed, custom-encoding, membership, DOCTYPE and child probes are unchanged. Six native C ASan/UBSan executions pass, with 336 allocation scenarios per linkage; Rust is uninstrumented and leak sanitizer is disabled.

The original shared implementation and its independent composed run are preserved under `../shared-dtd-definitions/` and `../shared-dtd-composed/`. The separately measured branch change is under `../shared-dtd-fast-path/`. In that four-way screen it removes the observed tiny-document regression; project ratios remain near parity. The ordinary branch is measurably faster in this build, but the exact compiler/code-layout cause is unresolved. No broad project speedup or new full fuzz/PBS campaign is claimed.

`source.json`, `report.json`, raw callback/API observations and source review identify this exact root build. Both libraries reproduce the author candidate byte for byte.

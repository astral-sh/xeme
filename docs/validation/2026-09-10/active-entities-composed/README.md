# Active entity indexes on the constructor stack

This checkpoint replaces repeated source/value-chain scans with counted, salted active-name indexes and removes per-attribute copies of inherited names. It preserves separate general/parameter and source/inherited views, full-name identity, error precedence, selected allocator ownership, transactional rekey, and custom-encoding recovery. It changes no default resource limits.

The composed runtime passes 312 core/adapter Rust checks, strict Clippy and formatting. All 4,740 original API observations match the constructor baseline: 4,077 pass and 663 fail. The 1,518 ordinary traces, 6,468 attribute cases, 372 custom attribute cases, 384 malformed attributes, 2,392 malformed text cases, 32 streaming controls and 36,456 custom-encoding conditions retain their prior results. Position-only attribute differences remain visible.

The additional 4,680 membership conditions retain 360 conservative-depth outcomes differing from Expat; successful events match. All 4,104 DOCTYPE cases match. The 1,944 missing-parameter cases retain 24 error-code and 162 event differences; six child-context probes match. Six native C ASan/UBSan executions pass, including 333 selected-allocation scenarios per linkage. Rust is an uninstrumented release build and leak sanitizer is disabled.

`source.json` identifies the exact release libraries and original build commit. `restack.json` records the later standalone lockfile repair; every frozen runtime source hash remains identical. `report.json` summarizes the composed checks. The original author package is retained separately under `../active-entity-indexes/`. The higher C entity limits, shared DTD definitions, and new broad performance/fuzz/PBS claims are outside this layer.

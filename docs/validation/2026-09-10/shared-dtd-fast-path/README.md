# Ordinary-document event fast path

This followup adds eight lines over the frozen shared-DTD source (manifest `24f85545`). Parsers with no table owner and no active DOCTYPE call the existing event body directly. The first DTD step, shared-family locking/publication, owned callback events, decoder state, limits and selected allocator remain as before. The full candidate source is archived; root owns integration separately.

## Validation

319 core/FFI release tests, including selected-allocation failure sweeps, pass. Strict release Clippy and formatting pass. The 4,740 public upstream observations are exactly unchanged: 4,077 pass and 663 fail. All 432 publication observations remain identical, including the nine known standalone callback-prefix differences. All 4,104 DOCTYPE handler/encoding observations match Expat; 15,288 custom-provenance observations have zero baseline changes, zero reference outcome differences, and zero successful callback differences. Failed upstream assertions remain in the logs. This layer adds no sanitizer campaign or general conformance claim.

## Timing

The four-way screen compares the pre-sharing baseline, frozen shared implementation, selected plain split, and unselected explicit inline/outline variant. Each has the same complete normalized callback preflights. Every measured parser sample matches an independently computed callback hash and element/text counts. Seven randomized process cohorts run on CPU0; each tiny-input process times 2,000 parses and each project process 30, after one warmup. The constructor-only probe times 200,000 create/free pairs after 1,000 warmups, without parsing or per-iteration timers.

Selected split/shared median paired ratios: empty 0.854/0.864, attributes 0.960/0.984, and text 0.928/0.982 (non-namespace/namespace). Six project ratios range from 0.986 to 1.022. Empty input versus the pre-sharing baseline moves from 1.097/1.114 to 0.947/0.968 in this screen. Constructor-only shared cost is about 3% over baseline. These are shared-host parser microbenchmarks; there is no broad project speedup claim.

The plain split still has the original register-save/80-byte stack prologue in this compiled library. The observed gain cannot be attributed to proven frame removal. Hardware perf sampling was denied by host paranoid4 policy; failed logs are preserved, with no host changes. The explicit outline variant is retained as an unselected comparison; only the plain split received the full release tests and final compatibility rerun.

## Artifacts

`fast.patch` is the isolated change. `source.json` and `source.tar.gz` identify every candidate source file; `parent-source.json` identifies the prior layer. `build.json`, compressed build/test logs, probe generators/observations, all native timing outputs, and the manifest preserve provenance. Local binary paths and hashes are recorded; binaries are excluded. Runner scripts retain the exact local evidence paths used in this session. `root-source-review.json` records the independent source review scope.

`benchmark-independent-review.json` records the independent audit of all 60 preflight streams, 392 process outputs and exact paired arithmetic. Successful runner return codes were asserted, but individual numeric codes were not retained. No additional run is claimed.

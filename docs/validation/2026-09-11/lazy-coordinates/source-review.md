# Lazy-coordinate runtime source review

No actionable runtime correctness blocker was found in the six runtime files sealed in the companion JSON. The review is against parent `de859c89577b2982d59b6923d682032f6a7c7800` and applies only while those exact file hashes match. It does not establish a speedup or production readiness.

## Reviewed invariants

- Only positive-count native unconverted UTF-8 root frames enter the new C coordinate seam; empty starts, pending sequences, DTD/shared state, children, CDATA and generic fallbacks keep eager publication paths.
- NativeLocation has private fields and binds parser generation plus exact byte start/count. The current-publication resolver rejects stale and foreign descriptors. Ordinary AdapterFrame.position rejects unresolved descriptors in release builds.
- Source keeps committed A separate from prospective B. The existing successful delivery decision commits B, including deliverable ordinary Text before a non-OOM accounting error. Early failures discard B and preserve A; entry recovery materializes and discards a leftover prospective point.
- Before existing source compaction, materialization resolves A then B then the consumed cursor. Resolved old points survive buffer discard; coordinate cursor rebases with the consumed cursor. No extra source bytes or deferred compaction are introduced.
- Ordinary Source and Parser position queries project exact line/column from the checkpoint. position_at and position_cursor carry the projected CR state, including split CRLF. Original byte accounting stays eager and the scalar input-context accessor preserves entity anchors.
- Public feed materializes after rejection guards and before decoder mutation. Encoding-map and conversion operations materialize before mutation. Ordinary delivery and generic consume remain eager; errors retain real Position values.
- The C dispatcher installs the event descriptor before callbacks. Coordinate getters borrow only the core field briefly, cache a resolved C Position, and retain allocator-reentry guards. Byte/index/context getters do not resolve. Existing detached Start/End owners and input-context Text pointers do not borrow the core across callbacks.
- C construction, reset, owned events, EOF, decoding requests and error publication store explicit Positions. Encoding release during reset observes the old core and publication before replacement.

## Roles and validation scope

Runtime implementation belongs to `/root/lazy_coordinates`. `/root/coordinate_review` independently reviewed that source and separately authored `crates/oriole_expat/src/coordinate_tests.rs`; the reviewer did not independently author-review its own tests. Those three tests cover byte-only versus repeated exact getters, suspended/reset-release visibility of the old publication, and rejection of a stale same-generation C descriptor. Their saved first run passed.

The reviewer ran no compiler, parser, test, profiler or benchmark target. The source review also caught an unsupported limit-setting order in an implementer-authored fixture; it was corrected before first execution. Final tests, Miri and performance acceptance remain with their respective execution lanes. Allocation-test strengthening and CI edits are outside this seal.

## Layout cost

Saved baseline DWARF and current `size_of` output establish Source 384→520 B (+136), Parser 2,408→2,416 B (+8), AdapterFrame 256→264 B (+8), and CParser 3,160→3,176 B (+16). This is not allocation-neutral. See `/tmp/oriole-lazy-coordinate-layout-readback.json`, SHA-256 `9312d8c30e064e8afa5cf2733b42e8d077fb61f86b67e96fee471feb4c6b1282`.

# External DTD declaration grammar

External parameter references can now load separate DTD children between declaration grammar tokens. The parent retains its lexical frames and resumes after importing child declarations. Completed attributes, reserved entity names and parsed identifiers become visible at Expat's callback boundaries. Enumeration and notation callbacks track handler changes without changing the full stored attribute type.

The independent isolated review includes 8,254 UTF-8/UTF-16 comparisons across every byte width, a 4,656-case grammar/mode/handler matrix, 4,500 pure-cursor acceptance probes, and earlier composition/header replays. No outcome or successful normalized-callback regressions were found. Reports retain malformed partial-Default, diagnostic and fragmentation differences; overlapping matrices are listed separately. Owned state, selected allocation, bounded cursor stacks, raw projection, child cloning and callback boundaries received separate source review.

## Combined runtime

The root integration preserves the namespace, XML version, foreign-DTD, API-state and encoding corrections through PR58 (`e23b9a3`). Only two test-context conflicts required manual insertion; runtime changes applied without conflicts. The combined shared library SHA256 is `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`; the static archive SHA256 is `4fa096d9ffdfcf2c01d764ae83190ab689064e10a87860822d6301d04da62651`.

| Gate | Result |
| --- | --- |
| Workspace tests and documentation checks | 246 pass |
| Formatting and strict workspace Clippy | Pass |
| Full Expat public API matrix | 3,753 pass, 987 fail; all 4,740 selected configurations complete, no signals/timeouts |
| Difference from the PR56 matrix | 12 encoding configurations newly pass; no regressions |
| W3C required acceptance checks | 5,916 pass, six fail |
| W3C optional/inconclusive observations | 81 optional; zero inconclusive/resolver errors |

The last external-grammar W3C descriptor now accepts at all three chunk sizes. The six remaining failures are `hst-lhs-007` and `rmt-e2e-38`, each at chunks 1/7/4096; reference Expat also accepts these catalog-required rejection cases. They remain failures. The W3C harness checks nonvalidating acceptance, not canonical output, using the previously documented pinned mirror and local-only resolver.

`isolated-review.tar.gz` retains the immutable implementation handoff, patch, full source hashes, independent reviews, generators and observations. `combined-checks.tar.gz` retains the exact combined source, merge deltas, build/test logs, complete Expat API results and W3C catalog/worker evidence. The API failures remain visible; this checkpoint does not claim full Expat compatibility. Fresh combined consumer, sustained sanitizer, benchmark and full distribution gates are recorded in subsequent checkpoints.

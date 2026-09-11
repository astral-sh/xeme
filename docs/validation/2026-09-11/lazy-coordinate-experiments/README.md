# Rejected lazy coordinate experiments

Both implementations are rejected. Every measured condition regressed against the capacity-fixed `de859c8` runtime, which remains selected. This change records the experiments and does not add their runtime code.

| Experiment | Build | Real XML / control | Generated / control | Adverse conditions |
| --- | --- | ---: | ---: | ---: |
| Deferred coordinates (`f766243`) | Normal | 1.0917× | 1.1141× | 28/28 |
| Deferred coordinates (`f766243`) | PGO | 1.0845× | 1.0747× | 28/28 |
| Skip unchanged checkpoints (`421bc18`) | PGO | 1.1097× | 1.0574× | 28/28 |

Ratios above one mean slower. Each campaign retained all 24 real-project conditions and four generated controls, with seven paired comparisons per condition. The real projects remained outside PGO training. Compiler settings and the generated training corpus matched the capacity-fixed control. Each package includes all individual results and raw sample reconstruction; aggregates do not hide adverse conditions.

## What we tried

The first version deferred line and column updates between eligible native UTF-8 C callbacks. It kept byte positions current and resolved exact coordinates when a getter, fallback or input compaction required them. Separate committed and prospective positions preserved the previous callback location when a new event failed. This added 136 bytes to each `Source`.

The second version removed empty coordinate scans, unchanged checkpoint stores and repeated writes of already resolved positions. It retained the same publication protocol and layout. A new regression covered unresolved positions exactly at the current checkpoint. This correction still regressed every PGO condition. Under the decision made before measurement, we rejected it without running normal-build or CPython timing.

The measurements establish regressions, not a complete attribution of their cost. Neither version justifies adopting the added state and bookkeeping.

## Evidence and limits

- [Deferred coordinates](v1/): 443 local tests and one doctest; both aliasing models in CI; normal and PGO native campaigns; exact differential checks; shared/static C and strict CPython consumers.
- [Checkpoint guards](v2/): 444 local tests and one doctest; independent source/build review; exact PGO differential checks; one PGO native campaign.

The first version's four strict CPython runs each preserved all 802 historical method outcomes, including the same two text-grouping failures. Those strict consumers include the documented allocation-failure cleanup backport. The second version did not repeat broad CI, strict CPython, full upstream API or PBS adoption gates. Neither version ran a CPython elapsed campaign.

The packages retain source snapshots, build settings, library identities, first outcomes, raw logs and independent reviews. Their portable readers check saved records without running a compiler or parser. Compiled artifacts and reusable build caches are retained by identity rather than copied into Git.

# Serialized parser family budgets

The private C parser-family counters use cells under the existing family-serialization contract. Matched C-only ThinLTO measurements reduce native project parsing time by 0.9% across all 24 conditions; actual CPython time is effectively unchanged. The full suite still takes 1.71× Expat's time natively and 1.31× through CPython.

See the [complete report](../../../../docs/validation/2026-09-11/family-budget-cells/) for all conditions, regressions, raw samples, source/compiler/library identities, compatibility results, independent reviews and reproduction controllers. Both elapsed Oriole libraries use ThinLTO without PGO.

# Remaining upstream failures

First-failing-assertion classification of101 remaining failed public test names; no failure waived and no production-ready equivalence claim. Current matrix3621pass/1119fail; reference4740pass under matched4GiB/15s limits. All failed candidate contexts exit100, no signal/timeouts.

| Classification | Test names | Failed contexts |
|---|---:|---:|
| allocation retry ceiling | 50 | 566 |
| allocation schedule or stage | 8 | 64 |
| documented unsupported api | 3 | 36 |
| exact error diagnostic | 12 | 144 |
| documented absolute limits | 5 | 50 |
| deferral or allocation heuristic | 2 | 13 |
| documented input context | 2 | 24 |
| exact position | 2 | 24 |
| foreign subset state | 1 | 12 |
| acceptance or api contract | 4 | 48 |
| external encoding semantics | 5 | 60 |
| caller policy callback | 1 | 12 |
| documented name repertoire | 2 | 18 |
| documented custom encoding | 3 | 36 |
| identity api | 1 | 12 |

## Highest-impact follow-ups

1. **Foreign DTD read acknowledgement and NotStandalone before EndDoctype/root**: Restores configured rejection and undefined-entity handling; isolated fix authorized by root.
2. **External general-entity decoder initialization context**: Closes acceptance difference for BOM-less ASCII/NUL UTF16LE and valid explicitLatin1 BOM-like input; preserve document/DTD handling.
3. **Small completed-parser/zero-length-buffer API compatibility**: Permit documented finished-state SetEncoding and zero-length ParseBuffer finalization when internal input buffer exists; retain positive-length reservation checks.
4. **Finish DTD lexical error precedence and positions incrementally**: Reduces diagnostic failures; lower priority than caller policy and decoded content semantics.

Fresh selected retry512 run620pass/64fail over684contexts; changes numeric retry ceilings only in old preserved C adapter and preloads exact final release, dladdr verified. This diagnostic never changes original3621/1119 passcount. Constructor extra probe: first success10 mallocs versus Expat5; original bound excludes10.

No crash, timeout, memory-safety failure, or missing absolute limit established by remaining assertions. Caller-policy omission and external encoding interpretation are real compatibility risks. Independent allocator sweeps/ASan campaigns remain separate evidence.

Per-test assertion evidence and exact classifications are in `failure-classification.json`.

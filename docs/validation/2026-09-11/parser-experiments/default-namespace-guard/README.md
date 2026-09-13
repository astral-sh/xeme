# Default-namespace lookup shortcut: unselected

The one-line runtime shortcut did not improve the real-project aggregate. Normal was **0.69% slower** and PGO **0.39% slower** than the selected Context runtime. The selected runtime remains unchanged.

| Build | Real ratio vs control | Adverse real conditions | Generated ratio vs control | Adverse generated conditions |
|---|---:|---:|---:|---:|
| Normal | 1.006900 | 15/24 | 1.034406 | 4/4 |
| PGO | 1.003900 | 14/24 | 0.986236 | 0/4 |

The candidate PGO/Expat real ratio was **1.364977** in this same campaign. That is a separate paired comparison, not the product of the two other aggregate ratios. Every condition and all three ratios are retained in `conditions.csv`; full seven-pair values and raw samples are archived.

## Change and checks

On the native-root Start-frame path, a namespace table containing only the required built-in `xml` binding cannot have a default namespace. The candidate checks `namespaces.len() == 1` before the existing empty-key lookup. It retains all other frame and namespace checks. An added regression verifies that an external child with a sole default binding still expands names and uses the fallback; the root invariant must not be generalized to children.

The uncommitted two-file candidate is based on `ef0eac091999c2e29e888918ac958480dbdacb4c`; source 72 is `0823604d…`. The selected measured control is Context runtime `0f28139f…`, source 72 `8a7da275…`, normal `eed1ee24…` and PGO `7cd7c5a8…`. Its allocator provenance repair remains byte-identical. `source.patch` and the exact source archive preserve the candidate independently of Git history.

All 438 local tests, formatting and strict all-target Clippy passed on the first attempt. The independent source review found no blocker under the existing native-root eligibility conditions. Fresh normal/generate/use builds passed, with nine actual compiler vectors matching the selected controls after the established private-path normalization and 864 generated records matching exactly. Original-G training, compiler settings, headers and native worker code are unchanged.

## Measurement and roles

Each build mode uses the original three-engine native callback driver: 24 real conditions (six projects, namespace off/on, 4 KiB/64 KiB) and four generated controls, seven shuffled pairs, seed 2026091003. The two campaigns contain 168 preflight workers plus 1,176 timed workers: 210,840 measured samples, 1,176 timed warmups and 336 preflight samples. Every callback digest/count, sample index, median, shuffled order and paired ratio passed saved-data reconstruction.

Root collected the builds and elapsed results. `current_pbs_review` prepared the current build adapters; `next_hotspot` prepared the native adapters and independently recomputed the raw arithmetic using a separate reader. The latter also authored the earlier selected Context build controller, so it does not claim independent authorship of every inherited controller. The candidate source review came from a separate agent.

No fresh Python elapsed or full compatibility/sanitizer/Miri campaign was run for this unselected optimization. Instruction-profile motivation was not an elapsed-speed prediction. No performance mechanism is inferred from these small regressions.

## Portable evidence

`evidence.tar.gz` contains content-addressed objects and an index of original paths/hashes. It retains candidate source, patch, six exact build helpers, compiler/training records, local checks, current audit/controller attempts, complete native protocol/preflight/result bytes, selected comparison inputs, original XML fixtures and their licenses/notices. Compiled artifacts are omitted with identities. Reusable target caches and redundant older review archives are excluded.

All 1,344 original worker files were independently checked and then byte-compared with reconstruction from the full result containers. `worker-aliases.json` records the exact reconstruction; these are verified byte aliases, not assumed JSON-value equivalences. Raw reports are not reserialized or replaced.

Run `python3 -I -S verify.py --package . --output readback.json` from a relocated package. It validates archive objects, exact source 72 contents and all worker aliases, then executes only `replay_native.py` against temporary saved JSON files to rebuild all 56 condition ratios and aggregates. It loads no parser or compiler and makes no new claim about live tool/library bytes. The original local audit separately checked those identities. The required original tools and source/input hashes remain recorded for reproducing the actual build/benchmark on a suitable host.

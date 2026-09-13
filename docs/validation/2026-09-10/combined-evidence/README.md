# Validation evidence — 2026-09-10

This package records completed checks and their limitations. It does not establish complete Expat compatibility. Original failures, skipped tests, optional observations, and development reproducers remain included. No executable libraries or compiler outputs are bundled.

## Source boundaries

| Evidence | Exact source or artifact |
|---|---|
| Combined parser, CPython consumers, native tests, full upstream matrix | Runtime at `b68bdca6f61e932c45ce076d6ec0bdb7aadd95b4`; source manifest SHA-256 `cc0f008866bc6b76c1a7cba72cb7b245d2a4eb26275a8928ead7dbd723afdd3b`; release `.so` `f7e602e300daebedfc08854ed32adebeb656165cddc019dcc0b0226063f9f569`, `.a` `eeeb554fed0e40d3ac2d6c96ecbb3736a30d64fdeeddeeaa9e9888ed7005a0df`. |
| Completed ASan campaigns | Same parser snapshot plus callback-family fuzz target; campaign source manifest `577f164cf357c529b386c5db2194e2e442bba8916b2c8f75d36db17c022a94b2`. |
| PBS build and glibc 2.17 execution | Earlier commit `6a6a9eca5f30e1bae05737148de4a0de793fdc7c`, successful [workflow 34442760577](https://github.com/astral-sh/oriole/actions/runs/34442760577). Exact compiled sources are in `pbs/oriole-bundle/manifest.json`. This is integration evidence for that earlier runtime. |
| Later XML version review | Isolated release `28c99a9520689339588cde62a83235cd7b5c9270beb1321e710214667e935d07`. |
| Later foreign DTD review | Isolated candidate `f8c07e08bd7b5960942ef2e5140d24209d3343eb0293e64a7dae7a6a83f407b2`, based on `b68bdca`; patch and source manifest retained. |

The completed main campaigns and full matrix predate the namespace, XML version, and foreign DTD follow-ups. They must not be presented as tests of a later combined revision. The later full PBS run was still in progress when this package was assembled and is not included as a success.

## Results

- **Upstream Expat 2.8.4:** 395 public test names × six input chunks × two deferral modes = 4,740 contexts. With matching 4 GiB address-space and 15-second per-test bounds, reference Expat passes 4,740; Oriole passes **3,621 and fails 1,119**, across 101 names. Every candidate failure exits through an assertion; no candidate signals or timeouts occurred. Compared with the recent 3,455-pass baseline, 166 contexts improve and none regress. Twelve private implementation tests are excluded by the preserved adapter, not counted as passes.
- **Original lower runner bounds:** candidate results are identical at 1 GiB/3 seconds. Reference has 14 runner-related failures at those bounds; these observations and the focused raised-bound replay remain in the package.
- **Failure classification:** `upstream/values/failure-classification.md` and its JSON list every first failing assertion. Most concern allocation retry ceilings or schedules, but caller-policy, external encoding, API, and diagnostic differences remain. A diagnostic retry-ceiling replay passes 620/684 contexts; it does **not** change the original 3,621/1,119 count.
- **CPython 3.12.13:** all six XML test modules succeed in four configurations: shared/static library and original/upstream allocation-fix consumer source. Each reports 803 tests, 31 skips, with expected failures retained in logs. The patched consumer backports only the recorded upstream `pyexpat.c` allocation-failure fix; no tests are changed.
- **Native C:** integration, adversarial callbacks, and allocator-failure checks pass for shared and static linkage (six runs). C consumers use ASan/UBSan; the linked Rust release artifacts are not themselves an ASan build.
- **Fuzzing:** three 601-second ASan campaigns complete without a crash artifact: `value_family` 913,601 executions; `multibyte` 2,297,235; `ffi_family` 1,029,737; total **4,240,573**. Seed/evolved corpora, commands, toolchain, limits, and 18 large-input replays are retained. `detect_leaks=0` is recorded; selected-allocator live-count assertions provide separate allocation evidence. This is a bounded campaign, not a proof of safety.
- **Combined declaration/value review:** the 18,468-case positive matrix has zero differences. Handler-switch matrices retain known malformed-input callback differences, with frozen-baseline comparisons; no successful-input or final-outcome differences remain in those matrices.
- **PBS:** archive validation, 22 custom tests, and six XML modules succeed. The produced interpreter also passes the six XML modules on pinned CentOS 7/glibc 2.17 (802 tests, 13 skips). TLS weak-reference/fallback checks and 32-thread destructor probes are preserved. The runtime source predates the main combined snapshot above.
- **Later follow-ups:** independent XML version/encoding matrix: 135/135 exact; the broader 360-case report retains malformed-declaration differences. Foreign DTD matrix: 4,632 final outcomes match, 1,866 newly exact cases, no previously exact regressions; 420 known diagnostic/Default differences remain. An additional 480 nested-read cases match exactly. Its focused upstream run passes 96/120 with 24 known failures retained.

## Layout and reproduction

- `runtime/`, `consumers/`, `native/`: source manifest, consumer commands/results, and native checks.
- `upstream/`: full candidate/reference results at both bounds, baseline, exact failure classification, diagnostic replay, and one copy of the adapter. Pinned upstream revision: `12cf0b1f25f026a022fe728ad8f7e3d017285b80`.
- `combined-review/`: owned-state/allocator review, oracle generators, complete final observations, and malformed baseline comparisons.
- `campaigns/final/`: immutable successful campaigns and source archive. `campaigns/development/` separately preserves the earlier no-Default quoted-value panic and its fixed-runtime validation. It is not a failure of the final campaign.
- `pbs/`: successful workflow metadata, archive/source hashes, build/test logs, and TLS review.
- `followups/`: separately hashed XML version and foreign DTD results.
- `harness-review/w3c/`: portable acceptance-runner reporting review and synthetic fixtures. These are harness tests, not a new complete W3C corpus result. Resolver failures are inconclusive and receive no conformance credit; timeouts and worker failures persist summaries.

`artifact-manifest.json` maps every stored artifact to its original path and SHA-256. Raw `.log` files and large JSON files are gzip-compressed; entries retain hashes of both original and stored bytes. Existing embedded manifests keep their original workspace paths, so use this mapping when locating archived files. The small campaign source archive provides the compiled runtime and harness source; large upstream source trees and binaries are represented by pinned revisions and hashes.

# Lazy coordinates V1: rejected experiment

**Experimental and rejected. Every one of the 56 measured conditions is slower than the capacity-fixed control.** This package preserves the negative result; it does not recommend adopting V1 or claim production readiness.

Candidate: `f766243bcc485ab305d339f5d37f095e0940eb49` ([draft PR136](https://github.com/astral-sh/oriole/pull/136)). Control: mandatory capacity fix `de859c89577b2982d59b6923d682032f6a7c7800` ([PR135](https://github.com/astral-sh/oriole/pull/135)).

## Native results

| Build | Real/control | Real/Expat | Generated/control | Adverse/control |
| --- | ---: | ---: | ---: | ---: |
| Normal | 1.091734292 | 1.736680654 | 1.114113088 | 28/28 |
| PGO | 1.084483637 | 1.486348376 | 1.074669675 | 28/28 |

Ratios above 1 mean slower. `conditions.csv` is the original, byte-exact 56-row audit CSV, including all individual project and generated results. Each build used the original 28 conditions (24 project and four generated, interleaved), seven paired comparisons and seed 2026091003. Geometric means aggregate per-condition medians of seven paired time ratios. One campaign per build; no selective reruns or elapsed benefit claim.

The archive retains all 1,344 workers and 212,352 samples: 210,840 measured samples, 1,176 timed warmups and 336 preflight samples. Original worker JSON bytes were checked against exact reconstruction from the retained containers before omission, not merely compared as JSON values. Every worker identity and reconstruction address is retained.

## Source and build evidence

The 73-file source snapshot, full source patch, six runtime-file changes and six test-file changes are retained. The matched build used O3, ThinLTO, one codegen unit and fresh normal/PGO outputs. The original generated-training definition and six Git-pinned pipeline helpers were retained; nine full compiler vectors were compared with the capacity-fixed control, normalizing only private paths/artifact suffixes. All 864 generated training records per build matched.

Local checks retain 443 tests plus one compile-fail doctest, then the stronger test-only allocation witness and its focused test, Clippy and formatting checks. The full suite was not rerun after that test-only edit. First outcomes remain visible, including the initial DTD compilation failure and the initial saved build audit with its incorrect control caption, followed by the caption-only correction and saved-reader rerun.

Build/native controller adaptation and saved-result auditing were performed by the same reviewer, separately from runtime authorship and root collection. This is not an independent-author controller review. The separate source/correctness reviewer authored only the three C-coordinate tests.

## Correctness and CI scope

- Thirty saved commands: both builds matched all 3,318 exact callback/status/error/location cases against the immediate capacity-fixed parent; all 12 C consumers passed.
- Four strict CPython runs each retain 802 method outcomes and the same two failures: `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Each exited 2. Three expected failures and 14 skip records remain visible (five methods, eight subtests, one class setup). Classification is compared with historical Context results, not a fresh strict run of the capacity-fixed parent.
- Strict consumers apply only the pinned upstream CPython allocation-failure cleanup backport. No test edits, text-fragmentation waiver or allocator override. ASan/UBSan instrument C consumers only; Rust release libraries are uninstrumented and leak checks disabled.
- CI run 34621512017 at the candidate commit passed all 16 jobs. Four saved Miri logs contain 96 tests: 48 under Stacked Borrows and 48 under Tree Borrows, including all three new coordinate cases in each model. Existing exposed-provenance/deprecation warnings remain in the raw logs. The exact workflow is retained.

No full upstream API, full PBS, fuzz or CPython elapsed campaign was repeated for this rejected prototype. Exact local plan/execution, compiler logs, source reviews, strict raw logs and origins, differential rows, CI logs and parent-reported tool-session completion metadata remain distinguishable.

## Portable verification

Use Python 3.12 or later with assertions enabled:

```sh
python3 -I -S verify.py --package . --output /tmp/oriole-v1-rejected-readback.json
```

The reader checks the file manifest and every archive object, reconstructs all worker bytes, validates the 73-file source archive, recomputes every native sample/median/ratio and all 56 CSV rows, and rechecks saved correctness command streams, differential rows, strict method/skip maps and CI result counts. It does not load a parser, compiler or binary and does not require any original absolute path. Source/build audit scripts and proofs are retained evidence; this portable reader does not rerun the original local build audit.

`evidence.tar.gz` uses the existing PR134 SHA256 object archive format. `archive-index.json` maps logical paths to deduplicated objects and original locations; `worker-aliases.json` records exact worker reconstruction. Compiled libraries, executables, extensions and shared caches are excluded from archive bytes, with pinned identities retained. No archive is blindly extracted. Corpus and Oriole/Expat/CPython notices are included. `files.json` binds the reviewable package files; verification receipts are produced separately.

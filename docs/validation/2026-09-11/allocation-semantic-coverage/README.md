# Current-source allocation semantic coverage

All **72 diagnostic configurations pass** on the selected reference-frame PGO library: six upstream tests, six chunk widths (0–5), and both reparse-deferral modes. Their existing text and callback assertions are now covered by a fresh run of the selected source. These cases previously had only historical raised-ceiling evidence.

The original API matrix remains **4,347 passes, 391 assertion failures, and two timeouts across 4,740 configurations**. All 72 corresponding original configurations still fail their original allocation-retry limits. This diagnostic changes six retry constants in a separate copied consumer and uses the larger historical diagnostic resource limits; it does not replace those failures or change the runtime, upstream harness, or accepted-failure list.

## What the successful retries cover

| Upstream test | Retry ceiling, original → diagnostic | Existing post-ceiling assertions reached |
| --- | ---: | --- |
| `test_alloc_public_entity_value` | 50 → 512 | Entity-declaration callback flag is set. The external loader also accepts only its original system/public-ID routes while parsing the nested declaration fixture. |
| `test_alloc_nested_groups` | 20 → 512 | Start-element text equals `doce`; element-declaration flag is set. The declaration callback frees the content model. |
| `test_alloc_notation` | 20 → 512 | Both notation and entity-declaration callback flags are set for the long-notation fixture. |
| `test_alloc_public_notation` | 20 → 512 | Notation callback flag is set for the long-public-ID fixture. |
| `test_alloc_system_notation` | 20 → 512 | Notation callback flag is set for the long-system-ID fixture. |
| `test_alloc_dtd_default_handling` | 25 → 512 | Accumulated text equals nine LF bytes followed by `<doc>text in doc</doc>`; all 11 original handler flags are set. |

All six retain the checks that allocation failure initially prevents success and that a later retry completes before the raised ceiling. Every fixture, loop, handler, assertion, setup/teardown routine, and runner is unchanged. Only the six listed constants differ; each resulting selected body matches its historical diagnostic body exactly. The archived patch contains the complete source change.

These are the upstream tests' existing oracles. Dummy declaration handlers prove callback presence; they do not check exact argument values, invocation counts, ordering, positions, or content-model structure. Retry loops stop at the first successful parse and do not establish exhaustive OOM coverage or exact error codes for every failed allocation. In particular, success at 512 does not prove that the original failure was caused solely by an insufficient retry count: diagnostic time and memory limits also increased.

## Resource limits

| Limit | Original full API matrix | Separate diagnostic |
| --- | ---: | ---: |
| Per-context time | 3 s | 15 s |
| Per-child address space | 1,024 MiB | 4,096 MiB |
| Sampled per-child RSS | 768 MiB | 3,072 MiB |
| Whole test invocation | 240 s | 300 s |

The copied runner enforces address space with `RLIMIT_AS`, a child alarm for per-context time, and a parent RSS check about every ten 1 ms polls. Sampled RSS is not an exact peak-memory bound. The controller kills and reaps the process group on its overall timeout. The separate C compilation has a 120-second limit. No limit was increased after observing this run.

## Provenance and review

The root agent ran the diagnostic once on CPU 3 outside elapsed campaigns. Compilation and execution both exited zero. The C consumer used the original `-O1 -DXML_TESTING -D_GNU_SOURCE` command with remapped paths; no Rust compilation was needed. Neither this C consumer nor the Rust PGO library was sanitizer-instrumented.

- Measured source: `0f66d54ac8418f0a6e628ad18570677a19c9ed45`, published in [PR #144](https://github.com/astral-sh/oriole/pull/144) at `4064b0653534aff690c0e0f395a6b3b48da878fc`.
- Selected library SHA-256: `d3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4`, matching the selected build manifest and original full API matrix.
- Existing `dladdr(XML_Parse)` output identified the staged selected library in the actual test process. The controller checked that all preserved original API files remained unchanged before and after the run.
- A saved-output review reconstructed all 72 successes and 72 original failures from raw logs, checked the complete 4,740-result matrix, compared all adapted sources, and independently rehashed the local binary, compiler, libraries, and original API tree. That reviewer prepared the controller but did not execute the diagnostic; this is a separate readback, not an independent author review.

[report.json](report.json) is the original execution report. [review.json](review.json) records the saved-output review. `records.tar.gz` contains raw logs, preparation/execution scripts, original results and failure blocks, source/build manifests, both versions of the changed C file, and all copied C source/header inputs. It excludes executables and libraries. `records.files.json` indexes every archive member by SHA-256 and byte length; `files.json` indexes the top-level packet.

Run the portable saved-record check with `python3 review.py`; it only reads the archive. Its default output verifies the saved evidence without requiring the original `/tmp` files or loading a parser. `--local /tmp/oriole-reference-frame-semantic-diagnostic` additionally repeats the local hash checks when the original files are available. The archived preparation and execution scripts retain their original absolute paths for provenance and are not a standalone portable build recipe.

The September 10 result remains historical: all 72 selected records passed on library `ff5d86621114e464a0b20f108b6308fe155d9e8c87bd764bc00872cf124750f8`, as published in `docs/validation/2026-09-10/final-runtime/allocation-diagnosis.tar.gz`. The new result closes the source-age gap for these six cases only. It supplies no new performance, PBS deployment, sustained sanitizer, or general production-readiness evidence.

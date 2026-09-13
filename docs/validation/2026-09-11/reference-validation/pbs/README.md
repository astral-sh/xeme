# Selected reference-frame stable PGO PBS trial

The selected reference-frame runtime built through python-build-standalone with stable Rust and fresh PGO, passed the distribution validator, and completed 1,024 threaded parses on glibc 2.17. Both strict XML gates still fail on the same two callback assertions. The workflow remains failed. This refreshes the deployment evidence for the selected runtime; Oriole remains experimental, and this trial adds no performance measurements.

The [PBS run](https://github.com/astral-sh/oriole/actions/runs/34653585245), attempt 1, was explicitly dispatched at `4064b0653534aff690c0e0f395a6b3b48da878fc`. All 69 PGO source pins match measured reference-frame runtime `0f66d54ac8418f0a6e628ad18570677a19c9ed45`. The build uses CPython 3.12.13 and PBS revision `a4553880293fe9d1bb62747d34ab0e5121d3554f`.

## Outcomes

| Gate | Result |
| --- | --- |
| Fresh stable PGO bundle and CPython build | Passed |
| Complete distribution validator | Passed |
| PBS custom suite | 22 run: 17 passed, 5 skipped |
| Installed parser identity | Passed |
| Host strict XML suite | 802 initial tests, 2 failures, 12 reported skips; built-in retries repeated both failures |
| glibc 2.17 identity and threaded probe | Passed: Oriole identity and 1,024 parses |
| glibc 2.17 strict XML suite | 802 tests, 2 failures, 13 reported skips; subprocess exited 2 |

The failing methods are `test.test_pyexpat.BufferTextTest.test1` and `test.test_sax.CDATAHandlerTest.test_handlers`. Their exact assertion payloads remain in the raw logs: one expects different text callback grouping; the other expects surrounding newlines in the delivered character data. The host harness reports 806 executions and four failures because its built-in retry mechanism executes four methods and repeats both failures. These are two distinct failing methods.

The host harness enables `all,-largefile,-audio,-gui` resources; the old-glibc script uses defaults. These aggregate logs do not identify every skip or expected failure and do not establish a complete 802-row outcome map. The individual distribution validator passed; the broader “Validate the complete distribution” workflow step failed on its strict XML suite.

## Source, PGO and shipped identity

The saved-data reviews verified 73 bundle inputs, 69 PGO inputs, six pipeline files, ten successful command records and six actual workspace compiler vectors. The complete source/support union contains 87 files. Stable Rust 1.98.1 and LLVM 22.1.8 compiled both phases for x86-64 GNU Linux with optimization level 3, ThinLTO, one codegen unit, PIC and unwind. Ohm defaults and compiler wrappers were absent. The profile-use build supplied its own native static linker dependencies.

Fresh run `run-clljfotz` executed 288 cases in each phase from the original 12 generated fixtures; generate/use callback records match exactly. All generated inputs, raw and merged profiles, command logs and training records are retained. The saved profiler output contains 641 functions, 17,719 blocks and total count 837,998,455. These are training observations, not elapsed benchmark results.

The shipped `python/build/lib/libexpat.a` has SHA-256 `b71dfdb0969de8c529e64a2c04b1e4f2243995afa10e16abf7838272e00e360e`, exactly matching the fresh profile-use archive. The distribution archive hash is `30df6e4693682f14ff639a5752f6453a7e6c5edc7c05e6a5eb78c743add79090`. The installed build notice matches the bundle manifest byte for byte. Saved ELF inspection found no dynamic libexpat dependency, a maximum referenced glibc symbol version of 2.17 and the expected unversioned weak TLS wrapper in both the executable and shared libpython. The remote container log records the actual glibc identity and threaded execution.

The consumer cleanup adaptation remains the narrow upstream backport to `Modules/pyexpat.c`, identified in the report; CPython tests were unchanged. Neither this follow-up nor its review changes parser source.

## Evidence and limits

The [report](report.json) retains the workflow failure, exact API artifact origins, compiler vectors, source boundaries and outcomes. The [independent review](independent-review.json) separately checks source/profile/training/log identities, API ZIP bytes, the shipped archive and selected distribution members. The [evidence archive](evidence.tar.gz) contains the original validation ZIP, all 42 nested raw records including failed XML logs and profiles, full workflow output, source snapshots, licenses/COPYING notices, ELF inspection and the original and independent readers. The [member index](archive-members.json) gives origins and hashes; [file hashes](files.json) identify the compact packet.

The 47,997,060-byte compressed distribution, downloaded executables/static libraries, compiler tools and build targets are excluded. Their exact available hashes remain in the reports and member inventory. Both ZIPs matched GitHub API digests; no separate signature or attestation was checked. Remote tool and non-shipped library identities remain producer hashes and recorded training origins. Full replay of the original artifact audit requires the original distribution and source checkout; the compact archive cannot recreate excluded binary checks.

Review used saved files and host data-inspection tools only. No downloaded interpreter, parser, compiler or container was executed locally for this report. This evidence does not show full Expat compatibility, refresh sanitizer coverage, establish performance within 10% of Expat, or establish production readiness.

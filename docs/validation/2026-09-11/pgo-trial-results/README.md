# Stable PGO PBS distribution trial

The first stable-toolchain PGO PBS build shipped the exact freshly trained Oriole archive and passed the distribution validator. The installed parser identified itself as Oriole, and the glibc 2.17 container completed 1,024 threaded parses. Both strict XML gates retain the same two callback assertions as the [normal stable PBS baseline](../pbs-independent-checks/); the workflow remains failed. This closes the PGO deployment-path check, without claiming fully compatible XML behavior or adding a performance measurement.

The [PBS run](https://github.com/astral-sh/oriole/actions/runs/34586223169) built PR126 `c61b63476ce83b69814fd704b6c6710811e52a9c`, checked out as merge `b9c4e8671dc04b154bae7c88393c6a110c3a7e67`. Its automatic PGO branch used stable Rust 1.98.1 and the matching LLVM 22.1.8 profiler. The separate [regular CI run](https://github.com/astral-sh/oriole/actions/runs/34586223118) passed all 12 jobs, including the 14 PGO and 6 bundle-provenance Python tests. No workflow was rerun or manually dispatched.

## Actual outcomes

| Gate | First-run result |
| --- | --- |
| Fresh PGO bundle and CPython 3.12.13 build | Passed |
| Complete distribution validator | Passed |
| PBS custom suite | 22 run: 17 passed, 5 skipped |
| Installed parser identity | Passed |
| Host strict XML suite | 802 initial tests, 2 failures, 12 reported skips; built-in retries ran 4 methods and repeated both failures |
| glibc 2.17 identity and threaded probe | Passed: Oriole identity and 1,024 parses |
| glibc 2.17 strict XML suite | 802 tests, 2 failures, 13 reported skips; subprocess exited 2 |

The failing methods are `test.test_pyexpat.BufferTextTest.test1` and `test.test_sax.CDATAHandlerTest.test_handlers`. The first compares text callback grouping; the second expects the surrounding newlines in separate text callbacks. Their exact assertion payloads match the normal baseline. The host report totals 806 executions and four failures because its own retry mechanism repeats tests; this is not four distinct failures. The host harness enables `all,-largefile,-audio,-gui` resources, whereas the old-glibc script uses the default resource set. The terse logs do not identify every skipped or expected-failure method, so we do not infer a complete per-method outcome map from these totals.

## PGO and shipped identity

The saved-data review verified 71 bundle source pins against the exact PR head, all 67 PGO source pins against the selected streaming runtime, six pipeline source pins, ten successful command records and six actual compiler invocations. Both fresh phases used explicit x86-64 GNU Linux targeting, optimization level 3, ThinLTO, one codegen unit, PIC and unwind. Experimental Ohm defaults and compiler wrappers were absent. The profile-use build supplied its own native static linker dependencies.

Each phase executed the same 288 generated cases from 12 fixtures, with identical callback records. The raw and merged profiles, generated inputs, compiler output and training records are retained. The profile report contains 632 functions, 17,635 blocks and a total count of 860,351,839; these are training observations, not elapsed benchmark results.

The shipped `python/build/lib/libexpat.a` has SHA-256 `37c412bf52ba1891cb0268cba74bb57394698cc846fdc9954e7407e5c0faffb9`, exactly matching the profile-use archive. The installed build notice is byte-identical to the bundle manifest. The distribution archive hash is `6186cbd58b6c502af5ffdb216255028ebe107e1cbcc8a6b5f7777f0ab4e4fdf3`. Saved ELF inspection found no dynamic `libexpat` dependency, a maximum referenced glibc symbol version of 2.17 and the expected unversioned weak TLS wrapper reference in both the executable and shared libpython. Actual container execution supports that linkage check.

## Evidence and scope

The [report](report.json) records the exact API artifact origins, compiler vectors, source boundaries, hashes and outcomes. The [independent review](root-review.json) separately checked source, profile, training, log and shipped archive identities. The [evidence archive](evidence.tar.gz) retains the original validation ZIP, raw failed XML logs within it, full workflow output, profiles, generated inputs, source pins, ELF inspection, audit readers and the regular CI receipt. Its [member index](archive-members.json) maps every retained file to its origin and hash; [file hashes](files.json) identify the published report files.

GitHub API ZIP digests were verified; no independent artifact signature or attestation was checked. The complete binary distribution, compiler/profiler executables and non-shipped instrumented/shared library binaries are excluded from this documentation package. The shipped archive and selected ELF bytes were read back locally before exclusion; remote tool and non-shipped library identities remain the producer hashes and training origins in the API-digest-verified evidence. The distribution member inventory retains selected binary hashes. Replaying the complete saved-data audit requires the original binary artifact, exact source checkout and recorded local paths; verifying the compact package alone cannot recreate those binary checks.

These results use the selected streaming parser runtime. The [earlier local Ohm bundle and C-consumer evidence](../pbs-pgo-bundle/local-ohm/) remains separate. No parser source changed, no downloaded interpreter was executed during review, and no new benchmark was run for this report. The two strict XML differences and the wider documented compatibility limits still apply.

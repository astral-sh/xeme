# Installed PR166 runtime: normal PBS trial and CI

The installed-distribution trial for [PR166](https://github.com/astral-sh/oriole/pull/166) completed with the two known CPython grouping failures. **PBS run [34712464503](https://github.com/astral-sh/oriole/actions/runs/34712464503), attempt 1, failed.** Archive structure, installed parser identity, custom checks and the glibc 2.17 threaded smoke test passed; the host and glibc XML suites failed. These results qualify the installed execution paths listed below and do not establish installed performance or complete compatibility.

PBS tested head `47776625cfee5cb5f0258eec93bbb1248209113c`, containing runtime `67c704c123b8661a3ad1f91bd48c2f0be4996096`. The [saved readback](result.json), [run metadata](metadata/pbs/015-run.json), [job steps](metadata/pbs/015-jobs.json) and [bundle manifest](bundle-manifest.json) retain the exact source and outcomes. This is current raw-view installed evidence; the [preceding PBS trial](../pbs-current-normal/) applies to the earlier outer-whitespace runtime.

## Installed outcomes

| Check | Recorded result |
| --- | --- |
| [Distribution structure](raw/pbs-validate.log) | Passed |
| [Installed provenance](raw/pbs-installed-provenance.json) | Oriole bundle identity verified by the workflow |
| [Custom checks](raw/pbs-custom-tests.log) | 22 tests, 5 skips, no failures |
| [Host XML suites](raw/pbs-xml-tests.log) | Failed: 806 executions including retries, 4 failure executions, 12 skips; two distinct failing methods |
| [glibc 2.17 execution](raw/pbs-glibc217-tests.log) | Identity and 1,024 threaded parses passed; XML suite failed with 802 executions, 2 failures, 13 skips and runner exit 2 |
| [TLS destructor probes](raw/pbs-tls-probe/manifest.json) | Native, static fallback and shared fallback each completed 32 thread destructors exactly once |

The host runner initially ran six XML modules, then retried each failed module with its failing test name selected. Each retry executed two methods, one passing and one failing: 802 initial executions plus four retry executions account for 806. The four recorded failure executions represent the same two methods, including their repeats:

- `test.test_pyexpat.BufferTextTest.test1`: character data `2\n3` arrives as one callback instead of the expected `2`, `\n`, `3` callbacks.
- `test.test_sax.CDATAHandlerTest.test_handlers`: the assertion receives `\nParseable character data\n` instead of `Parseable character data`.

Both assertions also fail in the glibc 2.17 run and match the current local strict shared/static failure names and assertions recorded in the [main qualification report](../shared-native-raw-text/). They remain failures. Full method-by-method equality to the local runs is not asserted: installed runners have environment-dependent skips and automatic retries. `test_xml_etree`, `test_xml_etree_c`, `test_minidom` and `test_pulldom` pass in both installed runs. The host XML log also retains a second successful 22-test custom-check invocation and the nonfatal `exec_prefix` warning.

## Build and source identity

The manual workflow used normal generic `x86_64-unknown-linux-gnu`, `target_cpu: null` and `pgo: false`. The bundle records stable Rust 1.98.1, O3, ThinLTO and one codegen unit, with PIC and unwind flags. Its three normal build vectors contain no target-CPU or profile-use flags. The PBS revision is `a4553880293fe9d1bb62747d34ab0e5121d3554f`; the consumer is CPython 3.12.13. All 76 manifest source hashes were checked against the actual PBS-tested Git head. [Host requirements](raw/pbs-host-cpu.json) and [bundle build output](raw/oriole-bundle/build.log) are retained.

The strict installed consumer includes only the existing `Modules/pyexpat.c` cleanup backport from [upstream CPython commit e6b9a140](https://github.com/python/cpython/commit/e6b9a1406980fbb1d4032eca9cc0b4f8f252b716), for [issue 144984](https://github.com/python/cpython/issues/144984). The manifest pins original, patch and resulting source hashes. CPython tests and other source files are unchanged by that backport. This installed qualification is separate from the unmodified CPython consumers used for the local timing comparisons.

The successful installed-identity step records static archive SHA-256 `11d6e08c3c3f945973329b2c3fae6dc545ddf5a94a50b7a5ab31d3428e96a215` and manifest SHA-256 `ddac0d4b3179e355354851d605123f524096c8d93ae5a9d25d5df7ecd9e8c726`. The [remote archive record](raw/pbs-archive-sha256.txt) identifies the experimental `noopt` distribution and its hash. The distribution and static archive were not downloaded or executed locally for this readback; their recorded identity is remote workflow evidence.

## CI and fixture corrections

[Final CI run 34713037741](https://github.com/astral-sh/oriole/actions/runs/34713037741) passed all 15 ordinary jobs at `008d818237fe0d6f8c04a000a98ad69ab5780ceb`; the optional PGO job was skipped. Both C callback aliasing modes passed four core and eight C tests. [Final metadata](ci/ci-test-fixed/completed.json), [Tree Borrows output](ci/ci-test-fixed/job-103605160938.log) and [Stacked Borrows output](ci/ci-test-fixed/job-103605160940.log) retain those checks.

| CI head and run | Outcome and correction |
| --- | --- |
| `47776625`, [34712437075](https://github.com/astral-sh/oriole/actions/runs/34712437075) | 13 passed, 2 failed, PGO skipped. Both callback jobs stopped at the expected-test inventory assertion before Miri tests ran. [Failure evidence](ci/ci/failure-cause.json) and the [one-line inventory fix](ci/inventory-fix.patch) are retained. |
| `de144aff`, [34712708782](https://github.com/astral-sh/oriole/actions/runs/34712708782) | 14 passed, 1 failed, PGO skipped. Tree Borrows found a stale userdata borrow in the test fixture after a direct read of its state. [Failure evidence](ci/ci-fixed/failure-cause.json) and the [fixture borrow renewal](ci/fixture-borrow-fix.patch) are retained. |
| `008d818`, [34713037741](https://github.com/astral-sh/oriole/actions/runs/34713037741) | 15 passed, PGO skipped; both Miri modes passed. |

The [exact commit scope](metadata/commits.json) and patches show that these follow-ups change only CI inventory and `cfg(test)` fixture code. Production runtime bytes remain unchanged. PBS stayed on its original `47776625` dispatch and was not rerun after either correction; its result does not cover the later fixture snapshot. Green CI does not replace or waive the failed installed XML suites.

## Evidence and limits

This report preserves compact raw validation logs, source/build manifests, completed run/job/check metadata, all three CI outcomes and the actual corrections. The [copy index](COPY_INDEX.json) binds each copied file to its saved source. [Artifact metadata](metadata/pbs/015-artifacts.json) and the readback retain the compact validation ZIP hash and every member hash. The full 10.7 MB PBS build log, TLS symbol dumps and experimental distribution remain in the workflow artifacts; the validation ZIP and extracted logs are also retained locally. Distribution artifact availability is subject to GitHub retention.

This normal installed trial makes no PGO, target-CPU, installed-speed, exhaustive memory-safety or production-readiness claim. The two strict grouping failures, original 391 API failures and failed W3C conformance run remain open. The [main report](../shared-native-raw-text/) retains the separate local timing and qualification scopes.

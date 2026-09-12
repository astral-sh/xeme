# Explicit x86-64-v3 PGO PBS trial

[Manual workflow 34660840404](https://github.com/astral-sh/oriole/actions/runs/34660840404)
built and validated the distribution's target and installed archive identity at
`9d4d268f9629cb2d528b3e11e781698e99d7d82d`. **The workflow remains failed on two
strict XML callback assertions.** All 16 separate PR150 CI jobs passed; the
PR-triggered distribution run was intentionally skipped.

| Check | Actual outcome |
| --- | --- |
| Explicit v3 CPU/OS guards, fresh PGO bundle and PBS build | Passed |
| Installed target, manifest and static archive provenance | Passed |
| Complete distribution validator | Passed |
| Custom distribution suite | 22 tests: 17 passed, 5 skipped |
| Host strict XML suite | 802 initial tests, 2 failures; 806 executions and 4 failures including retries; 12 skips |
| glibc 2.17 identity and thread probe | Passed, 1,024 threaded parses |
| glibc 2.17 strict XML suite | 802 tests, 2 failures, 13 skips; subprocess exit 2 |
| Installed supplemental text semantics on the local host | 2 passed, 0 skipped |
| Separate local v3 shared/static module supplements | 2 passed per mode, 4 total |

The two strict methods are `test.test_pyexpat.BufferTextTest.test1` and
`test.test_sax.CDATAHandlerTest.test_handlers`. Host retries repeat those methods;
they are not four distinct failures. The glibc identity/thread probe succeeds
before the strict XML subprocess fails. Original assertion text, skips and
workflow failures are retained. Aggregate logs do not provide a full method map.

The installed interpreter has built-in `pyexpat`, with no `__file__`. Its
supplemental wrapper pins the interpreter and unchanged `97b76ae0` test script,
checks the actual built-in origin and Oriole version, then runs both original
test methods. It replaces only the shared-extension-specific entry-point guard.
The semantic checks combine adjacent text while preserving every whitespace
character, buffering control and element/CDATA boundary. Their success is
separate from the unchanged strict failures. The local shared/static module
results use different already-built consumers and are labeled separately.

The archive is the actual `x86_64_v3-unknown-linux-gnu` PBS product; Rust keeps
`x86_64-unknown-linux-gnu` as its ABI target and explicitly selects
`target-cpu=x86-64-v3`. The readback verifies 75 bundle source pins, 69 PGO source
pins, six pipeline files, six actual workspace compiler vectors and 288 generated
observations in each of the fresh generate/use phases. Stable Rust 1.98.1 uses
O3, ThinLTO, one codegen unit, PIC and unwind behavior. The shipped static
archive exactly matches this run's PGO-use archive. Both inspected ELF files
reference glibc no newer than 2.17 and have no dynamic Expat dependency.

[Report](report.json) retains the original remote outcomes.
[Supplemental results](supplemental-semantics.json) retain the local execution
commands, supervising root session receipts and source hashes. The
[archive index](archive-members.json) binds every [saved-record member](evidence.tar.gz)
to its source bytes; all members and the nested validation ZIP were read back.
Source, license/COPYING notices, raw logs, generated inputs and profile data are
included. The 48,320,149-byte distribution and compiled binaries are omitted;
their exact API, archive and selected-member digests remain recorded.
Historical reader assumptions and corrections are preserved. Paths under `/tmp`
and `/home` identify the original machine; archive member names are relative.

This is an opt-in CPU-specific deployment trial, with generic defaults unchanged.
It adds no performance measurement or production-readiness claim. Strict
compatibility gaps and the broader performance goals remain documented separately.

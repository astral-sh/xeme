# Reuse the CDATA terminator searcher

**Retain the reusable searcher.** PGO native and CPython time each improve by about 0.5% across the 24 real-project conditions. The change preserves every measured compatibility outcome. Oriole remains slower than Expat overall: 1.37× its PGO native time and 1.13× its PGO CPython time. The production performance goal remains open.

## Implementation and source

Plain character data must reject `]]>` outside CDATA sections. The parser previously constructed a substring searcher for each text span. It now initializes one borrowed `memchr::memmem::Finder` through `OnceLock` and reuses it. Each search retains its local search state. The haystack, first-match result and ordering against invalid-character errors remain unchanged.

This is one hunk in `crates/oriole/src/lib.rs`, with no new dependency or parser field. Initialization remains lazy and is included in each benchmark process's existing warmup. The first use can contend between threads. The locked memchr version is 2.8.3; its borrowed needle does not require an owned allocation. Search dispatch also changes for short haystacks, so the measured effect cannot be attributed solely to constructor removal.

The 70-file source manifest is `69b96121a090467b3cea81eb25a4e627976f1a18c4cab628855c9880dd25f982`. Builds and checks used PR125 `144e69e08c5462217d54791374e579e3ef390a87` plus this hunk. Publication advances over PR127 `ee73ccdeefa9572666c753776ed309860a1057e4`; all 70 source files remain identical. That intervening stack adds PGO integration tools and documentation. The evidence preserves the actual earlier build scripts and compiler commands used for these measurements.

## Complete performance results

Ratios below are geometric means of per-condition median paired ratios. Lower is faster; every normal and PGO condition remains in the [performance evidence](performance/). The [decision](decision.json) includes all adverse conditions.

| Build and consumer | Conditions | Finder / parent | Finder / Expat | Faster than parent |
| --- | ---: | ---: | ---: | ---: |
| Normal native, real XML | 24 | 0.989832 | 1.587298 | 20/24 |
| PGO native, real XML | 24 | 0.994922 | 1.369664 | 16/24 |
| Normal CPython, real XML | 24 | 1.003800 | 1.269216 | 9/24 |
| PGO CPython, real XML | 24 | 0.994929 | 1.131862 | 16/24 |
| Normal native, generated controls | 4 | 0.967396 | 4.912457 | 4/4 |
| PGO native, generated controls | 4 | 0.976596 | 4.921072 | 4/4 |

Normal CPython regresses 0.38%, including a 0.78% ElementTree regression; its pyexpat aggregate is effectively flat. Eight real conditions regress in each PGO campaign. The largest PGO native regression is 2.33% for DocBook without namespaces at 64 KiB; the largest Python regression is 1.10% for DocBook pyexpat at 4 KiB. PGO beats Expat in six of 24 native conditions and four of 24 Python conditions. These results justify a small implementation improvement, not a claim that the parser is generally faster than Expat.

All four campaigns completed on their first attempts. Native campaigns use six original XML fixtures, namespaces off/on and 4 KiB/64 KiB chunks, plus four generated controls. Python campaigns use the same six fixtures, ElementTree/pyexpat and both chunk sizes. Each campaign uses seven seeded paired cohorts with the preceding parser and Expat as controls. Together they retain 2,496 worker executions and 265,080 samples, including warmups and preflights. Independent readers reconstruct the sample medians, pairing, output identities and adverse results.

Both parsers' PGO training uses generated XML, with the six project fixtures held out. Oriole uses Ohm 1.98.1-dev with experimental defaults disabled, LLVM 22.1.8, O3, ThinLTO and one codegen unit; Expat 2.8.4 uses GCC 13.3, O3 and no LTO. This is not compiler parity. Every Oriole phase retains its actual workspace compiler vectors, profiles and training outputs. The [rejected O2 experiment](rejected-o2/) remains separate and retains its 3.07% PGO regression.

These are XML parsing workloads, not full project executions. File reading and Python import/canonicalization stay outside the timed region; parser construction, parsing, callbacks and destruction stay inside. The existing native driver uses explicit library paths and file hashes. Python additionally verifies the active `XML_Parse` origin in each worker. Shared-host effects and run-to-run variation limit interpretation of these small differences. The main README's six-row display is independently recomputed in [readme-benchmark.json](readme-benchmark.json).

## Compatibility and review

| Check | Result |
| --- | --- |
| Workspace | 420 tests and one doc test pass; strict all-target Clippy and formatting pass |
| Original upstream API matrix | 4,347 pass, 391 assertions, two timeouts; all 4,740 rows byte-identical to the parent |
| C consumers | Six consumers pass; dynamic/static allocation coverage retains 327 scenarios per linkage |
| Strict traces and custom aliases | 3,318 strict traces and 36,456 custom comparisons retain baseline outcomes |
| Malformed inputs | 2,392 complete observation pairs match the parent |
| Callback publication | 1,304 cases / 3,912 parser executions retain baseline behavior |
| End-event lifecycle and allocation failures | 38 lifecycle cases and six allocation scenarios retain baseline behavior |
| Strict CPython shared/static | Both retain the same 802 method maps and two callback-grouping failures |

The [compatibility evidence](compatibility/) retains commands, unchanged bounds, raw failures, source/library identities and separate root and collector-owned audits. The API test bounds remain three seconds, 1 GiB address space, 768 MiB resident memory and a 240-second aggregate timeout. The two strict failures remain `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`; no test expectation or text-fragmentation override changed. Each linkage also retains three expected failures and 14 reported skips: five method skips, eight subtest skips and one class-setup skip. Strict consumers use the same approved upstream CPython allocation-failure cleanup backport as their baseline. Timing consumers use the original unmodified CPython source for all three engines.

The six native consumers instrument C with ASan/UBSan; the tested Rust PGO library is uninstrumented and leak checking is disabled. The earlier [streaming ASan and large-input campaigns](../streaming-input-bounds/) and [stable PGO PBS distribution](../pgo-trial-results/) predate this searcher change. They are historical evidence for the preceding runtime. No fresh full Rust ASan campaign, large-stream campaign or installed PBS distribution benchmark is claimed here. Successful custom-alias raw pairs were not archived by the unchanged harness; their controllers, counts and difference summaries remain available.

Instruction-count diagnostics found reductions of 2.12%/2.04% on Vulkan without/with namespaces and 1.17% on Wayland. Those runs used one warmup and one measured parse, despite inherited iteration captions. They measure instructions within `XML_Parse`, not elapsed time. PGO text size grows by 0.90%, and substantial inlined code moves between functions. The diagnostics support investigating the change but do not replace the complete elapsed results above.

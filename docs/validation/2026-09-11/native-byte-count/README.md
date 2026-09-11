# Native source byte counts

For UTF-8 and ASCII input, the number of original source bytes is already the stored byte count. We move the converted-encoding calculation into a separate private helper and inline the small native check. The existing debug assertion and both calculations remain unchanged. This lets source consumption and position lookup avoid an out-of-line call for native input.

The change reduces elapsed time by **3.1% across 24 native project conditions** and **1.5% across 24 CPython conditions**, relative to PR #115. Generated controls improve by **4.0%**. Oriole still takes **1.59× Expat's native time**, **1.27× its CPython time**, and **4.99× on generated controls**. The broader performance goal remains unmet.

## Implementation and identity

Runtime `be22a271f8a5c004d13e515cd4024a68afe57887` is stacked on PR #115 (`079fb66`). Its six added lines affect only `crates/oriole/src/encoding.rs`. There are no new allocations, unsafe blocks, counters, fields or API changes. Encodings that require conversion use the original raw-width calculation; accounting, errors, callback publication and exact positions stay in the same phases.

The prototype used parent runtime `1ff7b64`, before PR #115's Windows-only test cast correction. Integration on `079fb66` inherits that correction. All 70 source files are recorded in [source.json](source.json), manifest SHA-256 `735afb9491158c3a34fd376109edc2312727382b5fa61e5e8b084796d7294f1d`. The committed runtime patch is identical to the prototype patch. A fresh integrated release produces **byte-identical shared and static libraries** to the measured prototype. The [independent source/build review](source-build-review.json) checks the committed patch, archived sources and actual compiler invocations.

| Artifact | PR #115 control | Byte-count candidate |
| --- | --- | --- |
| Shared SHA-256 | `02fcab59f6e3818c128da6706d67987ae7450dd8b59e97cd28ce497a547f28f5` | `ccfb22956a11c1b241ec755f46245c61f088187ef93143b77a7afec748983821` |
| Static SHA-256 | `69ebed414946cbaef1a940c052a636be277c809902943d676e824deab45d22d5` | `f58d5bc6ed28077933fcbcb84ad876cc7208e0102c9008347223da91a328b03c` |

Both elapsed Oriole builds use effective C-only ThinLTO and one code-generation unit, without PGO or allocator substitution. Three actual workspace compiler vectors match after only private paths are normalized. Builds use Ohm Rust 1.98.1-dev (`f62703110`), LLVM 22.1.8, `cargo +ohm -Zohm-defaults=no`, one job, no incremental compilation and empty compiler wrappers/flags. CI uses the repository's normal toolchain. The measured prototype omits two optional Ohm trust settings relative to the control; the integrated rebuild retains that environment. Compiler vectors match after private paths, and integrated libraries are byte-identical to the prototype.

The C artifacts use the explicit `cargo rustc --release --locked -p oriole_expat --lib --crate-type cdylib,staticlib` target override. Normal Rust workspace tests are a separate build. Expat 2.8.4 remains commit `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, library SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`, with its original GCC 13.3 `-O3` configuration.

## Native project measurements

This display selects 4 KiB chunks and disabled namespaces from the complete suite.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 51.224 ms | 29.384 ms | 1.71× |
| Wayland protocol | 1.205 ms | 1.077 ms | 1.12× |
| Maven POM | 0.819 ms | 0.439 ms | 1.87× |
| Batik SVG | 0.132 ms | 0.134 ms | 0.98× |
| GTK UI | 0.359 ms | 0.213 ms | 1.69× |
| DocBook XSL | 0.271 ms | 0.187 ms | 1.44× |

| Group | Conditions | Candidate / PR #115 | Candidate / Expat | Lower than PR #115 | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project XML | 24 | 0.969330 | 1.590945 | 24 | 1 |
| Generated controls | 4 | 0.959894 | 4.993940 | 4 | 0 |

The fixed suite parses six pinned original project inputs with namespaces enabled/disabled and 4/64 KiB chunks, plus four generated conditions. Seven seeded, shuffled rounds retain all conditions. All 84 preflights pass; 588 timed workers form 196 complete comparison groups, containing 105,420 measured parses and 588 excluded warmups. No condition regresses against the parent in its paired median. One native condition beats Expat.

Timing includes parser construction, handler registration, incremental feeding, finalization, callback hashing and parser destruction. File loading, dynamic loading and output checks are outside the timed interval. Ratios are medians of paired process ratios; aggregate ratios are their geometric means. Displayed times are medians of process medians. [native-summary.json](native-summary.json) retains every condition and all ratios.

## Actual CPython measurements

Unmodified CPython 3.12.13 extension sources (`3bb231a6a5dc02b95658877318bf61501a7209e9`) are compiled against all three frozen libraries. Six fresh extension build commands and same-process library/module origins are checked. Complete canonical outputs agree before timing. ElementTree and pyexpat event collection each parse six inputs at 4/64 KiB with namespaces enabled.

| Consumer | Conditions | Candidate / PR #115 | Candidate / Expat | Lower than PR #115 | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| ElementTree | 12 | 0.979446 | 1.343821 | 12 | 1 |
| pyexpat events | 12 | 0.989875 | 1.197855 | 11 | 2 |
| Combined | 24 | 0.984647 | 1.268741 | 23 | 3 |

DocBook pyexpat at 64 KiB regresses by **0.59%**. All 72 preflights, 504 timed workers, 168 comparison groups, 25,788 measured parses and 504 warmups are retained. Parser creation, feeding, finalization and explicit result destruction are timed. Imports, input preparation and canonicalization are outside timing. Automatic GC stays enabled; explicit `gc.collect` calls occur outside timing. [python-summary.json](python-summary.json) and the [independent audit](python-review.json) preserve the reconstruction and sole regression.

Both elapsed studies run serially on CPU 0 of a shared Linux AMD EPYC-Milan host. Affinity does not imply exclusive host use. These measurements parse project XML rather than complete application builds. Batik's external DTD is not loaded. No sample or condition is discarded, and small per-condition differences remain subject to shared-host variation.

## Instruction and code-size evidence

The separate Callgrind study retains 32 preflights, 32 profiles and 128 parser samples. All 12 project and four generated conditions use fewer instructions: geometric ratios are 0.977682 and 0.972419 respectively. Static disassembly verifies that the old `raw_len` call is gone; native paths use the encoding check and existing count while converted paths call the extracted helper. Shared-library `.text` grows from 784,461 to 787,405 bytes (**+2,944 bytes**). These observations support the mechanism but do not isolate every cause of elapsed-time changes.

## Compatibility and review

The integrated normal workspace passes **407 tests in 33 binaries**, one doc test, strict workspace Clippy and formatting. The prototype also passed 85 existing focused tests covering core behavior, encodings, multibyte widths and positions. No implementation-mirroring test was added for this behavior-preserving extraction.

The unchanged upstream matrix has **4,347 passing / 393 failing configurations**, with all 4,740 raw rows byte-identical to the prior baseline (`dcda1aff`). The 393 failures span 38 of 395 test bodies, repeated across chunking and deferral modes: 298 allocation retry ceilings, 44 realloc schedules, 12 allocations after empty ParseBuffer, 12 fixed-budget child creation, 12 literal version identities, 14 resource/harness bounds and one buffer-growth heuristic. The [classification](../../2026-09-10/detached-frames/api-classification/) remains authoritative. We do not change limits, allocation calls or expectations to improve the score.

Six C ASan/UBSan executions cover three consumers with shared/static linkage, including 327 selected-allocation scenarios per linkage. The linked Rust release libraries are uninstrumented and leak checking is disabled. Results remain exact across 3,318 strict traces, 36,456 custom-encoding comparisons, 2,392 malformed-input comparisons and 1,304 publication cases with 3,912 executions. The unchanged custom-encoding helper retains counts, commands, hashes and difference summaries, but not every successful raw pair.

All 38 End lifecycle cases preserve the prior results, including existing differences from Expat. Six End allocation cases retain four forced failures, two successes and zero live blocks. Those fixtures exercise namespace-restoration fallback; they do not alone establish coverage of every detached End path.

Strict shared/static CPython each execute **802 tests with exactly the same two failures**, `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`. Raw exits remain 2, with no fragmentation waiver. Four persistent-loader origins per linkage establish that the intended pyexpat and ElementTree extensions are used. All method outcomes match the original baseline; the six upstream test files remain unmodified. Fourteen reported skips comprise five methods, eight subtests and one class setup; three expected failures are separate. The [saved-outcome audit](cpython-root-review.json) was performed by the same root agent that collected these strict tests.

Source/build, profiles, native timing and Python timing have independent agent reviews. The integrated C gates also have an [independent saved-data review](gates-independent-review/README.md), while the workspace inventory review is author-owned. Final package/prose review is separate. Agents share this workspace, so this is not an external audit.

## Evidence and limitations

[summary.json](summary.json), [build.json](build.json), [workspace.json](workspace.json) and [gates.json](gates.json) record the identities and checks. [evidence.tar.gz](evidence.tar.gz) preserves sources, commands, compiler logs, raw measurements, review receipts and earlier unsuccessful setup attempts. Its 1863 direct members and 17 binary exclusions are indexed in [archive-members.json](archive-members.json); SHA-256 is `bc63cf87e4733c2b05e7391e0ef782747a6ba0ea1046efae92843a53485b6dd0`. The packager read back every direct member and its original. Nested handoffs retain their own detailed readback receipts.

The initial Python preparation collided with an existing scripts-only directory before building or running a parser. The actual study uses a new `python-root-study` directory. An initial End lifecycle controller stopped at a CPU-affinity assertion before any parser ran; a corrected controller ran only the remaining End probes. A postprocessing attempt lacked a saved classifier artifact and was corrected without rerunning parsers. These failures remain archived separately from successful results.

This source has no new sustained Rust sanitizer campaign or full PBS distribution build. Earlier PGO, allocator and PBS results apply to their recorded source versions. Oriole remains experimental, and this change does not establish production readiness or speed parity with Expat.

[Independent package review](package-independent-review/README.md) and [publication-text review](prose-independent-review/review.json) record the final review scope.

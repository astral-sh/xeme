# Current PGO and LTO validation

**Decision:** keep ThinLTO and evaluate the existing opt-in PGO workflow. On held-out real-project XML, PGO reduces Oriole's native time by 24.8% and CPython consumer time by 15.5%, improving every condition. Fat LTO alone helps less; fat LTO with its own fresh profile regresses native time by 6.8% and CPython time by 2.6% relative to ThinLTO PGO.

The runtime is `be22a271f8a5c004d13e515cd4024a68afe57887`, published in [PR #116](https://github.com/astral-sh/oriole/pull/116). This layer changes documentation and retains evidence; parser source, default build settings, allocator selection and resource limits remain unchanged. Oriole remains experimental and slower overall than Expat. See the [benchmark tables](../../../../benchmarks/results/2026-09-11/pgo-lto/).

## Matched builds and training

| Library | Configuration | Shared-library SHA-256 prefix |
| --- | --- | --- |
| Oriole normal | Explicit host target, ThinLTO, one codegen unit | `0c8c58750c02` |
| Oriole PGO | Same configuration, fresh generated-only profile | `4f1518fc819b` |
| Oriole fat LTO | Same source/target/compiler, fat LTO | `b86bb49a27b4` |
| Oriole fat LTO PGO | Fresh fat-LTO generation and training | `d4e911d2a914` |
| Expat normal | Pinned 2.8.4, GCC 13.3, `-O3`, shared, no LTO | `7a333bc8ac92` |
| Expat PGO | Same Expat configuration, verified generated-only profile | `12d33ad26315` |

The current Oriole build and training runs preserve the 70 runtime source files and six frozen PGO tool files. This publication updates the PGO guide after the experiment; its Python code remains unchanged. Local Oriole builds use Ohm 1.98.1 with experimental defaults disabled and LLVM 22.1.8; the matching `llvm-profdata` reports that same LLVM version. Actual verbose compiler commands establish C-only `cdylib,staticlib` final builds with effective ThinLTO or fat LTO and one codegen unit. Production builds should use the normal project toolchain, regenerate profiles, and repeat application validation.

The explicit-host normal artifact differs from the earlier implicit-host artifact `ccfb2295`. Performance comparisons use the matched explicit-host control. Changing the LTO configuration changes Cargo metadata and artifact suffixes; normal versus PGO within each LTO mode preserves metadata, with phase flags and artifact suffix changes recorded explicitly. No byte-identical build claim is made across those configurations.

Each Oriole LTO mode has its own new raw-profile directory, training run, merged profile and profile-use build. No ThinLTO profile is reused for fat LTO. Twelve deterministic generated XML fixtures cover encodings, namespaces, DTDs, references, attributes, comments, CDATA, nesting and line endings. Each phase checks 288 parse records; all match its normal control. All six real-project inputs remain outside training. The original production tool, profile-warning rejection and callback digests are unchanged.

The existing Expat normal and PGO artifacts were reverified against their 167 source files, compiler commands, generated input bytes and configurations, training records and profile files. Expat's historical interpreter identity was checked retrospectively; its training helper lacks a same-process `dladdr` check. Two inaccurate historical metadata fields are explained in the preserved review using the actual driver and records. They were not rewritten. Rust/LLVM and GCC are different compilers; matching applies within each parser's normal/PGO pair.

## Measurements and adverse results

The first native and CPython campaigns compare normal Oriole ThinLTO, Oriole ThinLTO PGO, normal Oriole fat LTO, normal Expat and Expat PGO. Separate three-engine campaigns compare ThinLTO PGO, fresh fat LTO PGO and Expat PGO. Every campaign keeps all six projects, both 4 KiB and 64 KiB feeds, seven seeded paired rounds, and all raw samples. Native conditions include both namespace modes; Python conditions include ElementTree and pyexpat. Native generated controls remain separate from the real-project aggregate.

Fat LTO without PGO regresses four of 24 native project conditions and seven of 24 CPython conditions, despite small overall improvements. Fat LTO with PGO regresses all 24 native project conditions and 20 of 24 CPython conditions. Its four Python improvements are the Batik conditions. Generated fat-PGO entity cases regress at both feed widths; rare-declaration cases improve. These results support retaining ThinLTO for the measured inputs and compiler.

With both parsers trained, Oriole takes 1.402× Expat's native time and 1.151× its CPython time in the first campaign. The separate fat-PGO comparison records 1.400× and 1.149× for the same ThinLTO PGO artifacts. Each campaign has four winning native conditions and four winning CPython conditions. These are parsing and result-destruction measurements on project XML, not complete application timings or a claim of statistical significance. The host is shared; CPU affinity does not control frequency, cache or memory-bandwidth contention. Concurrent work on other CPUs is recorded.

Independent agents reconstructed every raw process median, seeded order, paired ratio and aggregate, including all adverse conditions. Source/build/training reviews are distinct from timing reviews and from full compatibility checks. All reviews are by AI agents.

## Compatibility scope

The measured ThinLTO PGO library preserves all 4,740 original upstream API outcomes byte for byte: **4,347 pass and 393 fail**. The original failing suite exit is retained; assertions, retry ceilings and resource bounds are unchanged. Six C ASan/UBSan consumer runs pass, covering integration, adversarial callbacks and the original 327 allocation scenarios for each linkage. Rust release code itself is not sanitizer-instrumented in these C runs, and leak detection is disabled.

The 393 configurations represent 38 upstream test bodies run across chunking and deferral settings. Their existing classification is 298 allocation-retry ceilings, 44 specific reallocation schedules, 12 allocation-after-empty-buffer expectations, 12 fixed-budget child creation cases, 12 literal version-string assertions, 14 resource/harness bounds, and one buffer-growth heuristic. The [compatibility guide](../../../compatibility.md) describes the contracts and limits. These failures remain visible rather than being made green by changing the harness.

The same measured library also preserves 3,318 strict differential traces, 36,456 custom-encoding/external comparisons, 2,392 malformed cases, 1,304 publication cases across 3,912 executions, 38 closing-tag lifecycle cases across 114 executions, and the six selected closing-tag allocation scopes. Existing reference position differences remain recorded. The unchanged custom-encoding helper retains counts, commands, hashes and difference summaries; it does not save every successful raw pair.

Fresh strict CPython shared/static runs each execute 802 methods and retain exactly `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` as failures, with raw exit 2. Independent review verifies every method status and both failure bodies against the original strict baseline, along with unmodified CPython sources and four extension-origin records per linkage. Each run reports 14 skips (five whole methods, eight subtests and one class setup) and three expected failures. No text-fragmentation adaptation is enabled.

Latest full PBS distribution and sustained Rust ASan evidence still refer to earlier runtimes, as identified in the [preceding report](../native-byte-count/). This build study does not establish a general production replacement or complete the faster-than-Expat goal.

## Evidence

Compact summaries sit beside compressed evidence. Archives retain raw worker output, full compiler commands, generated fixtures, profile lineage, source snapshots, failed comparisons and independent review details. Executable libraries are excluded with exact hashes; compiler caches are outside the retained evidence scope. Compressed JSON files preserve the original bytes; decompress them before checking their recorded hashes.

| Evidence | Archive SHA-256 prefix | Direct members |
| --- | --- | ---: |
| [Normal/PGO build matrix and five-engine native campaign](native-five/handoff.json) | `d2a6b5926d33` | 1,525 |
| [Fresh fat-PGO proof and three-engine native campaign](native-fat-pgo/handoff.json) | `71a2717130fa` | 725 |
| [Both CPython benchmark campaigns](python/summary.json) | `972ce9832b4f` | 4,699 |
| [Measured ThinLTO PGO C/API checks](c-gates/handoff.json) | `a67f40ec0014` | 233 |
| [Strict shared/static CPython checks](strict-python/receipt.json) | `fc01522ca3bc` | 54 |

The five-engine native archive holds the shared source/tool snapshots. The fat-PGO native supplement refers to those snapshots; its readback's absent nested-source field is not a source mismatch. The Python package's 37 exclusion entries cover 34 unique paths, including three binary aliases and three repeated shared inputs. Each package records its own failed preparation or packaging attempts separately from successful target runs.

[Independent Python package review](python-package-review/review.json) checks every member and origin. Individual build, training, timing and strict-suite reviews are retained inside their corresponding archives. The final file index and C-gate review are retained alongside this report.

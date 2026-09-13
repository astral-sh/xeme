# Serialized parser family budgets

The C interface already requires callers to serialize access across a parser and its external-entity children. We use `Cell<usize>` for its three private family counters: input bytes, callback bytes and attempted child creation. Their checked read/update invokes no callback, allocator or destructor. This removes unnecessary atomic read-modify-write operations while preserving every charge site, limit and failed-charge outcome.

The change reduces elapsed time by **0.9% across 24 native project conditions**. Actual CPython elapsed time is **effectively unchanged**. Oriole still takes **1.71× Expat's time natively** and **1.31× through CPython** across these fixed conditions; the broader performance goal remains unmet.

## Scope and source identity

Runtime commit `1a4cae6669cd267fa51e0e104c3eac84edb3437d` applies the exact measured two-file patch above PR #113 (`89927e0`). The measured source snapshot is based on PR #112 (`1cb326c`); its only difference from this assembled source, among 70 tracked inputs, is the C interface README added by PR #113. [assembly.json](assembly.json) records every hash and the exact patch match. No parser, storage, header, dependency or Cargo configuration changes accompany the private FFI counter change.

The checked-add helper keeps inclusive limits and rejects overflow. A zero charge above an already exceeded limit still fails. Rejected charges leave the counter unchanged; successful charges remain consumed if subsequent allocation or parsing fails. Existing shared-budget tests retain their observations. Two new tests exercise the arithmetic boundaries and child-allocation failure, shared parent/child input and callback budgets, fresh root reset, and surviving children retaining their original family.

The core entity budget, allocation tracker, shared-owner reference counts and lifetime-token atomics remain unchanged. `Shared<T>` requires `T: Send + Sync` for its thread-sharing implementations; the private Cell-backed family no longer meets those bounds. We add no unsafe trait implementation or concurrent API. The existing C family-serialization contract continues to apply.

## Matched builds

Both elapsed Oriole libraries use effective C-only ThinLTO, without PGO. Local builds use `cargo +ohm -Zohm-defaults=no`, one compilation job, no incremental compilation, empty compiler wrappers and Rust flags, a distinct worktree target directory and workspace-specific shared intermediates. Each fresh release build records all three actual workspace compiler invocations. The candidate's normalized invocations match the control after changing only private paths. The compiler is Ohm Rust 1.98.1-dev (`f62703110`), LLVM 22.1.8. CI and production use the repository's normal toolchain.

```console
cargo rustc --release --locked -p oriole_expat --lib --crate-type cdylib,staticlib
```

| Artifact | Control | Cell candidate |
| --- | --- | --- |
| ThinLTO shared SHA-256 | `a55ca0f0fed68f8db0f2e8d3ea86c271c8491fe130b5074077aecace3ab4a4df` | `0cfd610876f1bfd26a4d37a7f86adc9961896dfe24177c180ec7d15e0bae04e9` |
| ThinLTO static SHA-256 | `3f4f51259327d2c736c5d93640e1b2dbb6de9e03ae0dc15fbc9f11df37c8aa60` | `c86ad85e232afe4f7947053e522dbc8517cdca3694740994c3d1da64540f0610` |

The candidate's separate normal shared/static builds are `99ec1371103191394785b807ef665091e593c2d1e8dd3807539c9de2d13f31a3` and `57d573c9d53107c998723eb9dbab5083036e87c356c79d2e1367f62e1fbff5d8`. Normal builds supply the Rust workspace gates and an exploratory instruction study; their results are not represented as Rust tests of the C-only artifact. Expat is the unchanged normal 2.8.4 reference at commit `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, shared SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`.

The normal instruction study retains 32 preflights and 32 Callgrind runs over 12 project conditions and four generated controls. Its project instruction ratio is 0.999873, effectively flat. Both rare-declaration conditions increase instructions; both entity controls are essentially unchanged. Instruction counts do not model the latency of locked atomic instructions. Only the subsequent matched ThinLTO elapsed study below establishes the small observed native benefit.

## Native project measurements

This display selects the six 4 KiB, namespaces-disabled conditions from the complete suite.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 55.779 ms | 28.985 ms | 1.93× |
| Wayland protocol | 1.259 ms | 1.074 ms | 1.17× |
| Maven POM | 0.944 ms | 0.448 ms | 2.13× |
| Batik SVG | 0.133 ms | 0.134 ms | 0.99× |
| GTK UI | 0.412 ms | 0.213 ms | 1.93× |
| DocBook XSL | 0.282 ms | 0.187 ms | 1.53× |

The fixed suite includes six pinned project XML inputs, namespaces enabled/disabled, and 4/64 KiB chunks, plus four generated controls. Seven seeded, shuffled rounds include every condition. Each worker measures parser creation, incremental feeding, finalization, callbacks and destruction after one excluded warmup. Input preparation and output comparison are outside the timed interval. Ratios are medians of paired time ratios, combined with geometric means. Displayed times are medians of process medians; dividing rounded display times does not reconstruct the ratios.

| Group | Conditions | Cell / control | Cell / Expat | Lower than control | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project XML | 24 | 0.991004 | 1.713753 | 21 | 1 |
| Generated controls | 4 | 0.984825 | 5.073209 | 3 | 0 |

Three project conditions regress against the control: GTK namespaces at 4 KiB (+0.072%) and 64 KiB (+0.186%), and Wayland namespaces at 64 KiB (+0.281%). Generated rare declarations at 4 KiB regress by 0.659%. The sole native Expat win is Batik at 4 KiB without namespaces (0.986434×, a 1.36% margin on this shared host). All 84 preflights, 588 timed workers and 196 complete comparison groups are retained, with 105,420 measured parses and 588 warmups.

## Actual CPython measurements

Unmodified CPython 3.12.13 XML extension sources (`3bb231a6a5dc02b95658877318bf61501a7209e9`) are compiled separately against each frozen library. Same-process origin checks verify the loaded `XML_Parse`; canonical outputs match before timing. ElementTree and pyexpat event collection each cover six project inputs at 4/64 KiB, with namespaces enabled. Imports, input preparation and output canonicalization are outside the timed interval. Parser creation, feeding, finalization and explicit result destruction are timed; automatic GC remains enabled, while explicit `gc.collect` calls are outside timing.

| Consumer | Conditions | Cell / control | Cell / Expat | Lower than control | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| ElementTree | 12 | 0.998171 | 1.396887 | 8 | 1 |
| pyexpat events | 12 | 1.000228 | 1.222951 | 5 | 2 |
| Combined | 24 | 0.999199 | 1.307029 | 13 | 3 |

These results are effectively flat against the control; we make no CPython speedup claim. All 11 regressions, ranging from 0.071% to 1.502%, are listed in [python-summary.json](python-summary.json). The three Expat wins are Wayland pyexpat at both chunk sizes and Wayland ElementTree at 64 KiB; the ElementTree margin is only 0.91%. The run retains 72 preflights, 504 timed workers, 168 complete comparison groups, 25,788 measured parses and 504 warmups.

Both elapsed studies run serially on CPU 0 of the shared Linux AMD EPYC-Milan host. We pin the processes without claiming exclusive use of the host. No samples or conditions are removed. These measurements parse project XML, not whole project builds; the Batik external DTD is not loaded.

## Compatibility and adversarial checks

The normal workspace passes **400 tests across 33 test binaries**, one doc test, strict workspace Clippy and formatting checks. Only the FFI unit-test inventory changes (88 to 90); all other binary counts match the previous runtime. The original commands, normal environment and 900-second per-command limits are preserved, with no failed or discarded runs. All saved source and normal/ThinLTO library hashes remain unchanged after these checks.

The C-only artifact retains **4,347 passes / 393 failures** across the original 4,740 Expat API configurations; every result row matches the control. The original bounds remain 3 seconds per configuration, 1 GiB address space, 768 MiB RSS and 240 seconds overall. The [classification](../../2026-09-10/detached-frames/api-classification/) explains allocation schedules, retry ceilings, resource bounds and literal version expectations. No allocation point, retry ceiling, version string or test result is adjusted to improve this count.

Six native C runs cover three consumers under both shared and static linkage, including 327 selected-allocation scenarios per linkage. The C consumers use ASan/UBSan; the Rust release library is uninstrumented and leak checking is disabled. Further gates preserve the baseline across 3,318 strict traces, 36,456 custom-encoding comparisons, 2,392 malformed-input comparisons and 1,304 publication cases (3,912 executions). All 38 End lifecycle cases retain their exact control observations and existing Expat differences. Six End allocation scenarios preserve four forced failures, two success controls and zero live blocks. Those six scenarios exercise existing namespace-undo fallback behavior; they are not general coverage of a future detached End path.

Separate shared/static CPython gates each execute 802 tests with the same two failures (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`), 14 reported skips and three expected failures. Raw exits remain nonzero. No callback-fragmentation waiver is used. The skip count includes five methods, eight subtests and one class setup.

## Evidence and limits

[summary.json](summary.json), [build.json](build.json), [assembly.json](assembly.json), [workspace.json](workspace.json), [gates.json](gates.json), [native-summary.json](native-summary.json) and [python-summary.json](python-summary.json) retain source identities, complete aggregates and gate outcomes. [evidence.tar.gz](evidence.tar.gz) contains the raw studies, source/build and gate handoffs, controllers, design/source reviews and independent validation reviews. [archive-members.json](archive-members.json) lists every direct member and omitted native binary with its original path and hash. The archive has 2801 direct members and 19 direct binary exclusions, SHA-256 `5dae7cbfb08171d24ac002e425e7c225edfee401129f323396d9480c8731c7fa`. Every member and original file was read back and checked; nested handoffs retain their own manifests.

Source, compiler/profile evidence, normal workspace results, C gates, both elapsed studies and strict CPython results were independently reviewed. The reviewer of the initial build/profile study disclosed authoring the two focused tests; the separate root source review covers those tests independently. The custom-encoding harness retains commands, source-loop counts, hashes, exits and empty difference summaries, but not full successful raw pairs. Independent review explicitly limits its reconstruction claim for those cases.

No new sustained Rust sanitizer campaign or complete PBS distribution is claimed for this runtime. The earlier campaigns and PBS results remain linked from the main README with their original source versions. The parser remains experimental, the 393 API failures and two CPython failures remain, and representative performance is still below Expat.

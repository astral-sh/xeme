from pathlib import Path
import json

repo = Path('/home/dev-user/code/oss/oriole-c-library-lto')
out = repo / 'docs/validation/2026-09-11/c-library-thinlto'
bench = repo / 'benchmarks/results/2026-09-11/c-library-thinlto'
table = (out / 'table.md').read_text().strip()
summary = json.loads((out / 'summary.json').read_text())
report = '''# C library ThinLTO

The release profile requests ThinLTO, but our ordinary library build also emits an `rlib`. The recorded final compiler command does not enable cross-crate LTO in that configuration. Selecting only `cdylib,staticlib` through Cargo makes the existing release setting take effect. We keep the manifest's `rlib` for Rust consumers and tests.

On the same `50c20c1` source, effective ThinLTO reduces elapsed time by **1.8% across 24 native project conditions** and **1.1% across 24 actual CPython conditions**. Oriole still takes **1.73× Expat's time natively** and **1.31× through CPython**. The performance goal remains unmet.

## Build and source identity

The matched local commands use `cargo +ohm -Zohm-defaults=no`, with one compilation job, incremental compilation disabled, empty compiler wrappers and Rust flags, a private worktree target directory, and workspace-specific shared intermediates. Production and CI use their normal Rust toolchain.

```console
cargo build --release --locked -p oriole_expat
cargo rustc --release --locked -p oriole_expat --lib --crate-type cdylib,staticlib
```

Both fresh builds record actual compiler invocations for all three workspace libraries. The C-only build's final invocation has `-C lto=thin`; its dependencies have `-C linker-plugin-lto`. This is Rust cross-crate optimization, not C/Rust cross-language LTO. The local compiler is Ohm Rust 1.98.1-dev (`f62703110`), LLVM 22.1.8. All 70 measured source files are identical; the production command layer subsequently changes the C interface README and build tooling, with no parser, storage, header, Cargo manifest, or dependency changes.

| Artifact | Normal | ThinLTO |
| --- | --- | --- |
| Shared SHA-256 | `9815cc852327d455c094b0fb72c1d33fb90a4571e67cf15965e96e2300376828` | `a55ca0f0fed68f8db0f2e8d3ea86c271c8491fe130b5074077aecace3ab4a4df` |
| Static SHA-256 | `92eafd42ffe68dc30db920ff5ff8d67357b2c6c4273f51b0e3e841b2bfb9ea69` | `3f4f51259327d2c736c5d93640e1b2dbb6de9e03ae0dc15fbc9f11df37c8aa60` |
| Shared bytes | 1,180,536 | 1,144,888 |
| Static bytes | 11,871,272 | 3,637,666 |

The normal artifacts reproduce the selected runtime's bytes exactly. Both shared libraries export the same 73 dynamic names/types; both static archives contain 299 native ELF objects and no residual bitcode. Expat is the same normal 2.8.4 reference, commit `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, shared SHA-256 `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`. This experiment changes only Oriole's C-library build; it is not a comparison of newly tuned builds of both projects.

## Native project measurements

The display below selects the six 4 KiB, namespaces-disabled conditions from the full fixed suite.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
TABLE

All 24 project conditions include six pinned XML inputs, namespaces enabled/disabled, and 4/64 KiB chunks. We retain all four generated controls. Seven seeded, shuffled rounds run one process per engine and condition. Creation, incremental feeding, finalization, callbacks and destruction are timed; each worker has one excluded warmup. Ratios are medians of paired ratios, aggregated with a geometric mean. Displayed times are medians of process medians, so dividing rounded display times does not reconstruct the ratios.

| Group | Conditions | ThinLTO / normal Oriole | ThinLTO / Expat | Lower than normal | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Project XML | 24 | 0.981899 | 1.726188 | 18 | 1 |
| Generated controls | 4 | 0.979883 | 5.170699 | 3 | 0 |

Six project conditions regress against normal Oriole by 0.016–0.779%: Vulkan 4 KiB namespaces, Wayland namespaces at both chunks, GTK 4 KiB without namespaces and GTK namespaces at both chunks. Generated entities at 4 KiB regress by 0.831%. The sole Expat win is Batik at 4 KiB without namespaces (0.993469×, a 0.65% margin). These small differences come from a shared host and are not a robust claim of general superiority.

The native run verifies 84 preflights, 588 timed workers and 196 complete comparison groups, with 105,420 measured parses plus 588 warmups. Input preparation and output comparison are outside the elapsed interval. This parses project XML, not whole project builds; the Batik external DTD is not loaded.

## Actual CPython measurements

The Python experiment uses unmodified CPython 3.12.13 XML extension sources (`3bb231a6a5dc02b95658877318bf61501a7209e9`), compiled separately against each frozen library. Same-process origin checks verify the loaded `XML_Parse`, and canonical outputs match before timing. ElementTree and pyexpat event collection each cover all six project inputs at 4/64 KiB with namespaces enabled. Import, input preparation and output canonicalization are outside the timed interval.

| Consumer | Conditions | ThinLTO / normal Oriole | ThinLTO / Expat | Lower than normal | Lower than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| ElementTree | 12 | 0.988796 | 1.412545 | 10 | 0 |
| pyexpat events | 12 | 0.989114 | 1.222712 | 11 | 2 |
| Combined | 24 | 0.988955 | 1.314205 | 21 | 2 |

The regressions are Wayland ElementTree at 64 KiB (+0.352%), GTK ElementTree at 64 KiB (+0.104%), and GTK pyexpat at 64 KiB (+0.128%). Both Expat wins are Wayland pyexpat. The run retains 72 preflights, 504 timed workers, 168 complete comparison groups, 25,788 measured parses and 504 warmups. A legacy raw method label says “Three normal parser libraries”; the recorded hashes and compiler invocations establish that the candidate is ThinLTO. We preserve that raw label rather than rewriting completed evidence.

Native and Python elapsed studies run serially on CPU 0 of the shared Linux AMD EPYC-Milan host. Their processes are pinned; we do not claim an otherwise idle or exclusive machine. No samples or conditions were removed.

## Compatibility and adversarial checks

The C-only artifact retains all **4,347 passes / 393 failures** across the original 4,740 API configurations, with every result row identical to normal Oriole. The original limits remain: 3 seconds/configuration, 1 GiB address space, 768 MiB RSS and 240 seconds overall. The raw nonzero result is retained. The [failure classification](../../2026-09-10/detached-frames/api-classification/) explains allocation schedules, retry ceilings, resource bounds and literal version expectations.

Six native C runs cover three consumers under both shared and static linkage, including 327 selected-allocation scenarios per linkage. The consumers use ASan/UBSan; the Rust release library is uninstrumented and leak checking is disabled. Further checks retain exact baseline behavior across 3,318 strict traces, 36,456 custom-encoding comparisons, 2,392 malformed-input comparisons, and 1,304 publication cases (3,912 executions). All 38 End lifecycle cases match normal Oriole; their existing differences from Expat remain. Six supplemental End allocation scenarios retain the four forced failures, two success controls, exact observations and zero live blocks.

Separate shared/static CPython gates each execute 802 tests with the same two failures (`BufferTextTest.test1` and `CDATAHandlerTest.test_handlers`), 14 reported skips and three expected failures. Raw exits remain nonzero. No fragmentation waiver is used. The same-source normal build has 398 passing workspace tests and one doc test in the [preceding report](../../2026-09-10/start-frame-integration/); those Rust results are not represented as tests of this C-only artifact.

## Evidence and limits

[summary.json](summary.json), [build.json](build.json), [gates.json](gates.json), [native-summary.json](native-summary.json) and [python-summary.json](python-summary.json) retain exact identities and all aggregate results. [evidence.tar.gz](evidence.tar.gz) contains the raw measurements, controllers, source/build and gate handoffs, and independent reviews. [archive-members.json](archive-members.json) lists every direct member and omitted native binary with its origin and hash. The archive has ARCHIVE_MEMBERS direct members and ARCHIVE_EXCLUDED direct binary exclusions, SHA-256 `ARCHIVE_SHA`. Every included member and its original file were read back and checked. Nested handoffs carry their own source and exclusion manifests.

Build/static inspection, the C gates, both elapsed suites and strict CPython results were independently reviewed. The outer [build review](build-review.json), [native review](native-review.json), [Python review](python-review.json) and [CPython review](cpython-review.json) are also available without extracting the archive.

The production command validation is reported separately in [command-validation.md](command-validation.md). Its PGO and PBS smoke builds are not the `a55` measured artifact and have no new elapsed-performance claim. The historical PGO report's unsupported unconditional ThinLTO wording is corrected, while its original artifacts and measurements remain unchanged. Its old nonverbose logs cannot directly establish whether LTO was present.
'''
report = report.replace('TABLE', table).replace('ARCHIVE_MEMBERS', str(summary['archive']['members'])).replace('ARCHIVE_EXCLUDED', str(summary['archive']['direct_binary_exclusions'])).replace('ARCHIVE_SHA', summary['archive']['sha256'])
(out / 'README.md').write_text(report)
(bench / 'README.md').write_text('''# C library ThinLTO benchmarks

Effective ThinLTO on the `50c20c1` parser reduces elapsed time by 1.8% across 24 native project conditions and 1.1% across 24 actual CPython conditions versus the identical normal build. Oriole still takes 1.73× Expat's time natively and 1.31× through CPython. This does not meet the overall performance goal.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
''' + table + '''

These six displayed conditions use 4 KiB chunks with namespaces disabled. The fixed suite retains both chunk sizes, both namespace modes, all generated controls and all regressions. Times are medians of process medians; ratios are medians of paired ratios. The sole native Expat win is Batik here, by 0.65% on a shared host.

The [complete report](../../../../docs/validation/2026-09-11/c-library-thinlto/) contains build commands, compiler and library identities, all aggregate results, raw samples, compatibility results, independent reviews and reproduction controllers. The measured libraries do not use PGO.
''')
readme = (repo / 'README.md').read_text()
start = readme.index('| Vulkan registry |')
end = readme.index('\n\nThese [native measurements]', start)
readme = readme[:start] + table + readme[end:]
readme = readme.replace('These [native measurements](benchmarks/results/2026-09-10/start-frame-integration/)', 'These [native measurements](benchmarks/results/2026-09-11/c-library-thinlto/)')
start = readme.index('The latest start-tag change')
end = readme.index('\n\nOptional [profile-guided builds]', start)
readme = readme[:start] + "Building the C libraries with effective ThinLTO reduces elapsed time by 1.8% across the 24 native project conditions and 1.1% across the 24 actual CPython conditions versus the identical normal build. Oriole still takes 1.73× Expat's time natively and 1.31× through CPython. One Batik native condition and two Wayland pyexpat conditions are faster than Expat in this measurement; the Batik margin is only 0.65% on a shared host. The [full report](docs/validation/2026-09-11/c-library-thinlto/) retains all conditions, samples and regressions." + readme[end:]
(repo / 'README.md').write_text(readme)
print('Wrote ThinLTO report, benchmark summary and README; command-validation.md awaits actual production-command results.')

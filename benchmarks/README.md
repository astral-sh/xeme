# Benchmarks

Start with the [performance iteration guide](HILLCLIMB.md). Its `hillclimb.py`
entrypoint freezes ordinary builds and compares baseline, candidate and Expat in
matched rounds, with separate real-project and generated summaries.

The [QName confirmation](../docs/validation/2026-09-13/qname-confirmation/) records
the latest merged optimization: two separate epochs improve on the qualified
runtime, with confirmation at 1.1739× Expat natively and 1.0504× through CPython.
Use a fresh build of `main` as the baseline for the next experiment. Installed
distribution qualification remains scoped to the earlier source documented below.

## Current normal-build results

The [combined Start/End, namespace and declaration report](../docs/validation/2026-09-12/native-start-end-namespace-declaration/) measures exact tested runtime `5d983f7e` against the raw-view baseline `67c704c` and normal Expat 2.8.4. Native real-project time is 0.9314× raw-view baseline / 1.1822× Expat initially and 0.9316× / 1.1841× in confirmation. CPython is 0.9625× / 1.0554× initially and 0.9609× / 1.0558× in confirmation; confirmation ElementTree is 1.0914× and pyexpat 1.0214× Expat. Both native and Python meet the current 20% aggregate target in both epochs.

All four epochs remain separate: 104 condition rows, 2,496 workers and 265,080 samples, with all 12 adverse rows across nine distinct conditions retained. Native confirmation wins 22/24 real conditions and Python 23/24. Native namespace-on remains 1.2352× Expat, generated controls 3.5118×, and entity controls regress 5.85–5.90% versus the raw-view baseline. Individual outliers are not waived. The [six README rows](../docs/validation/2026-09-12/native-start-end-namespace-declaration/combined/native-confirmation/readme-benchmarks.json) use confirmation 4 KiB/namespaces-off data: medians of seven process medians, with paired-ratio medians computed separately.

The report retains all-condition CSVs, compiler/source/library bindings, raw-reader arithmetic and shared-host observations. The runtime is selected for a controlled opt-in Linux CPython 3.12.13 trial after completed bounded qualification. The [compatibility guide](../docs/compatibility.md) retains known failures and the experimental scope; these measurements do not establish production interchangeability.

## Historical checkpoints

These records retain the exact source, numbers, goals and decisions from their own checkpoints. Their historical 1.10× goals are not the current 1.20× aggregate target.

The [outer-whitespace report](../docs/validation/2026-09-12/outer-whitespace-separated-dispatch/) records runtime `6320d7b7`, based on PR162’s `87acc5a9`. A transient native space/TAB proof avoids repeated scanning of long prolog/epilog whitespace; uncertain suffixes preserve the existing path, callback boundaries, eager coordinates and accounting order. The ordinary inner Text plan expression is retained.

The targeted probe and full original matrix preserve the three-second alarm: both chunk-0 `test_misc_input_2gb` rows change from selected-control timeout 14 to candidate pass 0. The full matrix records 4,349 passes, 391 failures and no timeouts, with all other 4,738 ordered outcomes exact. Six C consumers pass. Strict shared/static CPython each retain 802 methods, 809 rendered outcomes and the same two grouping failures (raw exit 2); both separate semantic tests pass per linkage. Separate diagnostics pass all 300 retry configurations after raising exactly 25 local retry maxima to 512, and all 12 buffer-continuation configurations after replacing exactly two second-empty-call allocation-result assertions with unconstrained observations. Remaining semantic/tail assertions stay unchanged. Retry probes use 15-second tests, 4 GiB address space and 3 GiB RSS limits; buffer probes retain three seconds, 1 GiB and 768 MiB. These uninstrumented consumers do not fix the original 391 failures or establish exhaustive OOM coverage; the buffer fixture is not a callback-text bytes/count oracle. Current-source Rust ASan passes all six harnesses through corpus replay and bounded exploration, with leak detection disabled.

Initial native candidate/control is 0.998987805 (1.294736586× Expat); confirmation native is 0.999574572 (1.295048228× Expat), with 12 confirmation adverse rows. Initial CPython is 0.994636448× control and 1.121234780× Expat; confirmation is 0.992608945× control and 1.118172263× Expat, with 19 wins and five adverse rows. Four epochs retain 104 conditions, 2,496 workers and 265,080 samples, without pooling. Native is roughly tied; the substantive change is the timeout fix. Host counters do not prove isolation or statistical significance. Both overall aggregates remain above the roughly 1.10× goal. The candidate is selected for the two original timeout fixes, with native timing roughly tied and modest Python gains. All 35 adverse rows remain visible across four separate epochs. The roughly 1.10× overall performance goal remains open. The [normal generic PBS trial](../docs/validation/2026-09-12/pbs-current-normal/) validates this runtime’s archive and threaded parsing on glibc 2.17; both installed XML suites retain the two known grouping failures. No installed-interpreter performance was measured.

The [buffered-delivery report](../docs/validation/2026-09-12/buffered-delivery/) records runtime `279a0bee`, based on PR161’s `5d340f50`. We copy validated literal attributes as one span only when the existing frame capacity fits, separate successful event delivery from error recovery, and append known UTF-8 directly when its guarded warm-buffer path is eligible. Original fallback/error ordering and caller allocator ownership remain explicit.

The initial native epoch observes 3.25% less real-project time against the selected empty-state runtime, remaining 1.2948× Expat; all 24 real conditions improve. Its generated aggregate is 0.09% slower, with both rare-declaration regressions retained. Initial CPython time decreases 1.50%, to 1.1165x Expat. Confirmation decreases real native time 3.36% and CPython time 1.57%, remaining 1.2969x and 1.1197x Expat. Native improves in 24/24 real conditions and CPython in 23/24. The pyexpat aggregate is 1.0705x and ElementTree is 1.1711x Expat; 6/24 individual Python conditions meet the 1.10x goal. All 7 adverse main condition rows remain in the accompanying CSV. Both aggregates remain above the goal. Four main epochs retain 104 conditions, 2,496 workers and 265,080 samples with no pooling. Shared-host counters and CPU0 affinity do not establish isolation or statistical significance.

The separate arena diagnostic uses one 4 MiB feed to retain the frame across a primer, warmup and 1,000siblings. Packed attributes take .8903× control time; whitespace-heavy attributes take .9919×. The latter has two slower paired observations and broad Expat ratios (.6181–.9762), retained beside its .6304 median. These two cases are excluded from real/generated aggregates and cannot isolate the three combined changes causally. At that checkpoint, the cross-feed cache was an unbuilt follow-up. The later `467483ad` candidate completed ordinary checks and the native screen, but was rejected: its real-project geometric mean was 1.002935× the buffered control, with improvements in 9 of 24 conditions. Python and confirmation benchmarks were not run. The cache remains unselected, and current C frames still end at each feed.

The table in the [buffered-delivery report](../docs/validation/2026-09-12/buffered-delivery/) uses only that runtime’s confirmation 4 KiB/namespaces-off rows. Initial Python builds two fresh candidate modules and reuses four controls; confirmation reuses all six modules and compiles none.

The [empty-state guard report](../docs/validation/2026-09-12/empty-state-guards/) records previously selected `66ca8e0b`, stacked on PR160’s `c8411446`. Three guards skip zero-byte accounting, an absent parameter-reference `take`, and an empty default-event queue’s initial drain. Positive-byte accounting and its atomics, populated branches, later drains and callback ownership retain their existing behavior.

Confirmation observes 0.86% less real-project native time and 0.90% less CPython time against the preceding eager runtime, remaining 1.3443× and 1.1372× Expat. The initial epoch separately observes 0.70% and 1.05% less time. Native confirmation improves in 22/24 real conditions; CPython in 19/24. Five Python conditions meet the roughly 1.10× goal; the pyexpat aggregate is 1.0936× and ElementTree is 1.1826×. All four generated cases improve in both epochs (6.49% initial, 6.07% confirmation). Both Wayland namespace-on native confirmation regressions and all five Python confirmation regressions remain explicit. Four separate epochs retain 104 conditions, 16 adverse rows, 2,496 workers and 265,080 samples; no observations or medians are pooled. Shared-host counters and CPU0 affinity do not establish isolation or statistical significance.

The source-specific project table in the empty-state report uses that confirmation epoch’s 4 KiB/namespaces-off measurements only: medians of seven process medians, with ratios computed separately as medians of paired ratios. Confirmation reuses all six benchmark extensions and performs fresh preflights without compiling modules.

The [eager bare-tag position study](../docs/validation/2026-09-12/eager-bare-tag-positions/) records previously selected `19251bc0` against the preceding attribute runtime. Confirmation observes 2.10% less real-project native time and 0.40% less CPython time, remaining 1.3581× and 1.1510× Expat. Native improves in 23/24 real conditions and CPython in 16/24; five Python conditions meet the roughly 1.10× goal. Its source-specific project table in the eager report uses confirmation 4 KiB/namespaces-off measurements only: medians of seven process medians, with ratios computed separately as medians of paired ratios.

The generated aggregate decreases 3.98%, while both rare-declaration cases regress 2.38% and 1.20%; Batik's 4 KiB/namespaces-off native case regresses 0.23%, and all eight Python confirmation regressions remain. The initial epoch separately observes 2.24% less native and 0.50% less CPython time. All four epochs retain 104 conditions, 20 adverse rows, 2,496 workers and 265,080 samples; no observations or medians are pooled. Before/during host observations and CPU0 affinity do not establish isolation or statistical significance. The local-proof follow-up is unbuilt and supplies no measurements.

The [quoted-attribute lane-mask study](../docs/validation/2026-09-12/attribute-lane-masks/)
records preceding `91038723` and the separately rejected compact-owner experiment.
Confirmation observes 2.39% less native real-input time and 1.60% less CPython time
against LF, remaining 1.3886× and 1.1535× Expat. Five of 24 Python conditions meet
the roughly 1.10× goal. Its source-specific project table remains in the attribute report. All three native real-input,
two generated and four Python confirmation regressions remain; generated time
increases 3.08%, including entity cases of 7.37% and 10.04%. The separate initial
epoch and rejected compact owner remain in the report. All five campaigns retain
132 conditions and 39 adverse rows, with no pooled samples or medians. Before/during
host observations and CPU0 affinity do not establish isolation or statistical significance.

The [SIMD Text lane-mask study](../docs/validation/2026-09-12/text-lane-masks/)
contains the preceding LF source’s normal-build measurements and validation records. Confirmation
observes 1.55% less native real-input time and 1.21% less CPython time against the
preceding namespace runtime, remaining 1.4248× and 1.1745× Expat. Every regression,
the separate initial LF epoch and the rejected fixed16 experiment remain recorded;
no observations are pooled. The host was shared: counters and CPU affinity do not
establish isolation or statistical significance. The preceding
[namespace study](../docs/validation/2026-09-12/sparse-namespace-storage/) retains its
own source and measurements.

## Running benchmarks

New Oriole performance work uses ordinary `-O3`, ThinLTO and one codegen unit; the
normal Expat control separately uses GCC 13.3 `-O3` without LTO. PGO experiments
have stopped; the existing workflow remains available only by explicit manual
request. Historical studies below retain their original build identities, values
and limitations.

See the [historical native and consumer measurements](results/2026-09-11/native-byte-count/),
[historical PGO/LTO measurements](results/2026-09-11/pgo-lto/),
[explicit C allocator measurements](results/2026-09-11/c-allocators/),
[earlier full runtime measurements](results/2026-09-10/final-runtime/),
[DTD runtime checkpoint](results/2026-09-10/dtd-checkpoint/),
[earlier combined runtime measurements](results/2026-09-10/combined-runtime/),
[inline text experiment](results/2026-09-10/inline-text/),
[earlier validated checkpoint](results/2026-09-10/checkpoint/),
[explicit namespace measurements](results/2026-09-10/namespaces/),
[callback allocation measurements](results/2026-09-10/callback-allocation/), and
[initial optimization comparisons](results/2026-09-10/README.md) for results, source
archives, raw observations, and limitations.

The native C driver compares the Expat-compatible ABI against system Expat with
identical element and character-data callbacks. The Python runner performs an
untimed comparison of complete normalized callbacks before measuring, shuffles
paired process order, and records every observation and source/input/library hash.

```console
python3 benchmarks/run.py --library /path/to/liboriole_expat.so \
  --output /tmp/oriole-benchmark
```

Each process measures ten complete parses after one discarded warmup. Creation,
handler registration, parsing, callbacks, and parser destruction are included.
Process startup, library loading, and input loading are excluded. The driver
hashes names, attributes, and text independently of text callback fragmentation.
The corpus contains elements, text, entity references, and prefixed names; the
default timing configuration disables namespace processing in both libraries.
Pass `--namespaces` to enable namespace expansion in both the complete callback
preflight and the timed C driver; the report records the selected mode. Keep
results from the two modes separate.
All numbers are generated-workload measurements on the recorded host.

## Allocators and the safe Rust API

`inprocess` is a separate workspace using the safe Rust parser API with the same
streaming callback digest. Build each configuration to a distinct target directory
and copy the resulting binaries before rebuilding. The default uses the system
allocator; `jemalloc` and `mimalloc` are explicit alternatives.

```console
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features jemalloc
cargo +ohm build --release --manifest-path benchmarks/inprocess/Cargo.toml --features mimalloc
```

The executable accepts `XML_FILE CHUNK_SIZE ITERATIONS` and emits JSON containing
one warmup and all measured samples. Keep results from distinct builds separate.
`allocation-counts` counts allocation/reallocation requests and requested bytes;
it does not count unique live bytes. It uses atomics and must be measured in a
separate run from uninstrumented timings. Library consumers choose their allocator.

Compare copied executables with randomized paired process order:

```console
python3 benchmarks/allocators.py \
  --binary system=/absolute/bench-system \
  --binary jemalloc=/absolute/bench-jemalloc \
  --binary mimalloc=/absolute/bench-mimalloc \
  --counts /absolute/bench-counts --inputs /absolute/xml-inputs \
  --build-manifest /absolute/build.json --output /tmp/allocator-results
```

The first executable is the baseline. Labels may also identify a parent and a
candidate build for an optimization. Both runners accept repeated
`--build-manifest` arguments and retain copies and hashes with their results.
Manifests should identify source hashes, the compiler, build commands, features,
and the resulting executable hashes. Build before measuring and keep source and
executables unchanged throughout each campaign.

`dtd_workload.py` generates separate declaration-only and repeated-element inputs
with 128–1,024 declared attributes. It exercises attribute type and ID lookup while
remaining below the parser's resource limits. The native callback digest omits
DTD declaration and ID metadata; deterministic regressions check those contracts
separately. Compare complete normalized callbacks outside the timed region too.

No benchmark result is a compatibility or production-readiness certification.

## DTD composition and incremental scanning

`dtd_composition_workload.py` generates declaration delimiters supplied by internal
parameter entities, conditional headers, repeated empty replacements, and long
quoted literals. `native_dtd_driver.c` adds an in-memory external DTD resolver and
declaration callbacks, including complete content model traversal and freeing.
The Linux runner compares exact serialized callback metadata before timing and
checks every timed digest and declaration/request count against that preflight.

```console
python3 benchmarks/dtd_composition_workload.py --output /tmp/dtd-inputs
cc -std=c11 -O3 -Wall -Wextra -Werror -I include \
  benchmarks/native_dtd_driver.c -ldl -o /tmp/dtd-driver
python3 benchmarks/dtd_scaling.py \
  --library /absolute/liboriole_expat.so --reference /absolute/libexpat.so \
  --inputs /tmp/dtd-inputs --driver /tmp/dtd-driver \
  --build-manifest /absolute/build.json --output /tmp/dtd-results
```

As with the other runners, the build manifest should identify the parser source,
compiler, commands, header and driver hashes. The report retains its hash along
with all samples, preflight metadata, worker failures, and input/library hashes.
The optional `--external-grammar` generator flag adds repeated empty external
references inside declarations; use it only for implementations supporting that
mode. Positions and Default callbacks remain separate compatibility checks.

## Real-project inputs and consumers

The [pinned corpus](projects/README.md) contains original XML from Vulkan, Wayland, Maven, Batik, GTK and DocBook. The runners compare native callbacks, matched CPython consumers and complete Wayland code-generation commands, validating all outputs before reporting timings. See the [baseline results](results/2026-09-10/real-project-baseline/README.md) and [reproduction commands](projects/RERUN.md).

## Historical profile-guided builds

The optional [PGO workflow](../tools/pgo/) builds an instrumented library, trains on
a fixed generated corpus, and rebuilds with a fresh profile. It preserves runtime
configuration and keeps real project XML out of training. Every run records source,
compiler, profile and library identities.

The [historical PGO/LTO study](results/2026-09-11/pgo-lto/) measures runtime `be22a27` with effective C-only ThinLTO and freshly generated profiles. PGO reduces native time by 24.8% and CPython consumer time by 15.5%; Oriole still takes 1.40× and 1.15× Expat's time with both parsers trained. Fat LTO without PGO helps little, while fat LTO with a fresh PGO profile regresses native time by 6.8% relative to ThinLTO PGO. All 24 project conditions per consumer campaign and the separate generated controls remain included.

The [earlier matched-build study](results/2026-09-10/version-consistent-pgo/) measures
runtime `4b11ace` through native callbacks, unchanged CPython consumers and the
Wayland scanner. PGO reduces Oriole's native time by 22.7% and Expat's by 13.4%;
with both trained, Oriole takes 2.23× Expat's time overall. The corresponding
comparisons are 1.55× for ElementTree, 1.30× for pyexpat's event interface and 1.25×
for Wayland code generation. All output gates, raw samples, independent arithmetic
reviews, invalid overlapping attempts and exact build identities are preserved.

The [initial paired study](results/2026-09-10/profile-guided-study/) improves Oriole
by 22.2% geometrically on held-out native project workloads. Expat trained on the
same generated corpus improves by 12.7%; the fair PGO comparison still takes 2.26×
Expat's time overall. Those measurements identify an earlier source and do not
certify newer builds. The [rejected optimization studies](results/2026-09-10/rejected-position-entity-studies/)
retain unsuccessful position and entity candidates, including memory regressions
fixed during review.

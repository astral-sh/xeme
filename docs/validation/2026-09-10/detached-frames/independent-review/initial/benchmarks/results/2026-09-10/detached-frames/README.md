# Detached callback storage: real XML benchmarks

The measured normal release takes **18.3% less time across 24 native conditions** and **8.2% less time across 24 actual CPython conditions** than the earlier `4b11ace` runtime. Every real-project condition improves. Oriole still takes **2.13× Expat's time natively** and **1.48× through CPython**. It wins only the two Wayland pyexpat comparisons, taking 0.935× and 0.932× Expat's time at 4 KiB and 64 KiB feeds.

## Native examples

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 70.569 ms | 29.019 ms | 2.43× |
| Wayland protocol | 1.648 ms | 1.081 ms | 1.52× |
| Maven POM | 1.212 ms | 0.439 ms | 2.75× |
| Batik SVG | 0.147 ms | 0.135 ms | 1.09× |
| GTK UI | 0.524 ms | 0.213 ms | 2.46× |
| DocBook XSL | 0.353 ms | 0.187 ms | 1.90× |

These six rows use 4 KiB feeds with namespaces disabled. Times are medians of seven process medians; ratios are medians of paired process ratios, so rounded times need not divide to the displayed ratio. Each process includes one excluded warmup. The complete report includes both 4 KiB and 64 KiB feeds and both namespace modes for every project.

## Complete aggregates

| Conditions | Count | Candidate / published | Candidate / Expat | Faster than published | Faster than Expat |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native real-project XML | 24 | 0.817× | 2.132× | 24 | 0 |
| Native namespaces disabled | 12 | 0.781× | 1.998× | 12 | 0 |
| Native namespaces enabled | 12 | 0.854× | 2.276× | 12 | 0 |
| Actual CPython XML consumers | 24 | 0.918× | 1.478× | 24 | 2 |
| ElementTree | 12 | 0.904× | 1.616× | 12 | 0 |
| pyexpat callbacks | 12 | 0.931× | 1.352× | 12 | 2 |
| Native generated adverse fixtures | 4 | 0.910× | 5.468× | 2 | 0 |

Aggregates are geometric means of all per-condition paired median time ratios. Lower is faster. The generated entity conditions improve by 17.2% and 19.1%; the rare-declaration conditions take 2.46% and 0.016% longer. All conditions and samples remain in the report.

## Inputs, consumers and protocol

The corpus contains original Vulkan registry, Wayland protocol, Maven POM, Batik SVG, GTK UI and DocBook XSL XML. The [corpus manifest](../../../projects/corpus-manifest.json) pins every input and upstream revision. The native driver records equivalent element/text output metadata. Batik's external DTD is not loaded. These runs parse project XML; they do not build those projects or run their complete applications.

The Python run compiles unmodified CPython 3.12.13 `pyexpat.c` and `_elementtree.c` for each of three engines with identical `-O2` flags and header bytes. Each worker explicitly loads both built extensions, verifies their paths and hashes, and resolves `XML_Parse` to the recorded parser library. The separate full CPython compatibility runs use a persistent finder to cover fresh imports. ElementTree timing includes tree construction and destruction; pyexpat timing includes the recorded callback consumer. Output checks preserve normalized text and event order. They do not assert identical text callback fragmentation.

Native timing retains 84 preflights, 588 workers and 105,420 measured parses plus 588 warmups. Python retains 72 preflights, 504 workers and 25,788 measured parses plus 504 warmups. Both use seven seeded, shuffled cohorts per condition, fixed iteration counts selected before this run, and the coordinated CPU0 lane on a shared Linux AMD EPYC-Milan host. Native seed is `2026091003`; Python seed is `202609104412`. No worker failed, timed out or was rerun. Independent audits reconstruct all orders, raw results, loaded identities, medians and aggregates.

## Sources and build identity

The comparison uses fresh normal builds with identical compiler settings and isolated intermediate directories. Expat 2.8.4 is pinned at `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, library `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478`. Published Oriole is `4b11ace`, library `6c64573061d9f4c8a0cc409f414f2314aa781b0baf30147388e06745872aaadc`. Candidate Oriole is library `9277b71c85f3c0345697010eb282a3be18a0144f25bc2f78e2a0cee20c175dcd`. The candidate includes the recent entity-origin and DTD-deferral fixes as well as all three grammar/frame layers; this is a combined-runtime comparison, not attribution to any single layer.

The final stack commit `5bc806e` retains an additional Rustdoc comment and test-only regression. Its library `5af2406b` has byte-identical executable code, headers, relocations and data except 18 panic-location line numbers and the build ID. The [source/build equivalence review](../../../../docs/validation/2026-09-10/detached-frames/) preserves the exact source differences and both full hashes. The measured library and raw sample identities are unchanged.

These are normal releases with the repository's release settings. There is no PGO, allocator substitution, dropped condition or full-application claim. The [earlier matched PGO study](../version-consistent-pgo/) applies to `4b11ace`; its relative gains are not extrapolated to this candidate. Earlier exploratory binaries whose build modes or intermediate freshness differed are retained separately and excluded from this comparison.

## Reproduction and evidence

[native-summary.json](native-summary.json) and [python-summary.json](python-summary.json) contain the exact aggregate values. The [validation archive](../../../../docs/validation/2026-09-10/detached-frames/) contains every raw sample, preflight, script, fixed protocol, source/build manifest and independent audit. Absolute paths record this host; reproducing on another host requires staging the recorded inputs and libraries and updating those path arguments while retaining counts, seeds and correctness checks. See the [benchmark guide](../../../README.md) for the supported repository benchmark commands.

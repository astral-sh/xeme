# Current PGO and LTO measurements

PGO reduces native Oriole time by **24.8%** and CPython consumer time by **15.5%** across 24 real-project conditions in each campaign. Every condition improves. The profiles train on generated XML; the six project inputs are held out.

| Comparison | Native time ratio | CPython time ratio |
| --- | ---: | ---: |
| Oriole ThinLTO PGO / normal ThinLTO | 0.752× | 0.845× |
| Oriole normal fat LTO / normal ThinLTO | 0.972× | 0.994× |
| Expat PGO / normal Expat | 0.867× | 0.944× |
| Oriole ThinLTO PGO / Expat PGO | 1.402× | 1.151× |

Lower is better. Ratios are geometric means of per-condition medians of seven paired process-time ratios. Native and CPython results come from separate five-engine campaigns. The four generated native conditions are reported separately: Oriole PGO/normal is 0.807× and fat/normal is 0.974×. No conditions or samples are dropped.

## Representative native conditions

| Project XML | Oriole PGO | Expat PGO | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 38.949 ms | 24.559 ms | 1.58× |
| Wayland protocol | 0.970 ms | 0.954 ms | 1.01× |
| Maven POM | 0.622 ms | 0.367 ms | 1.69× |
| Batik SVG | 0.106 ms | 0.125 ms | 0.84× |
| GTK UI | 0.270 ms | 0.178 ms | 1.51× |
| DocBook XSL | 0.196 ms | 0.164 ms | 1.19× |

These rows use 4 KiB chunks with namespaces disabled. Times are medians of process medians; ratios are medians of paired ratios, so dividing the displayed times need not reproduce the ratio. The complete experiment also includes 64 KiB chunks and namespaces enabled for every project.

## Fat LTO with PGO

A second native campaign trains a fresh profile for fat LTO and compares it with ThinLTO PGO and Expat PGO. Fat LTO increases Oriole's native time by **6.8%**, with regressions in all 24 real-project conditions. The generated aggregate increases by 4.6%; both entity conditions regress while both rare-declaration conditions improve. The matching CPython campaign increases time by **2.6%**, with 20 regressions and four Batik improvements. ThinLTO remains the preferred configuration for this workload set.

## Scope

All measurements use runtime `be22a271f8a5c004d13e515cd4024a68afe57887`, Linux x86-64 on a shared AMD EPYC-Milan host, the same native callback loop, and unmodified CPython 3.12.13 consumers. They measure XML parsing and result destruction, not complete project applications. The Batik external DTD is not loaded.

The first native campaign contains 140 preflights, 980 timed workers and 175,700 measured parses. The corresponding CPython campaign contains 120 preflights, 840 timed workers and 42,980 measured parses. The separate native fat-PGO comparison contains 84 preflights, 588 timed workers and 105,420 measured parses; its CPython counterpart contains 72 preflights, 504 timed workers and 25,788 measured parses. Every timed worker has one excluded warmup. Order is seeded and paired; CPU0 is reserved among the cooperating agents, but host frequency, cache and memory bandwidth remain uncontrolled. Build/preflight work on other CPUs overlaps some campaigns.

The [validation report](../../../../docs/validation/2026-09-11/pgo-lto/) records build identities, training, adverse results, correctness scope and archived raw evidence. The [PGO guide](../../../../tools/pgo/) explains reproduction. Existing non-PGO results remain in the [preceding report](../native-byte-count/).

# Position and string integration benchmarks

The combined position/string change reduces native project time by **1.8%** and
actual CPython time by **2.2%** versus the preceding bounded text-search runtime.
The overall times remain **2.01×** and **1.42×** Expat respectively. All four
native generated conditions regress; every sample remains included.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 68.445 ms | 28.993 ms | 2.36× |
| Wayland protocol | 1.412 ms | 1.071 ms | 1.32× |
| Maven POM | 1.149 ms | 0.443 ms | 2.62× |
| Batik SVG | 0.143 ms | 0.134 ms | 1.06× |
| GTK UI | 0.488 ms | 0.213 ms | 2.31× |
| DocBook XSL | 0.339 ms | 0.185 ms | 1.82× |

The displayed subset uses 4 KiB feeds with namespaces disabled. Times are medians
of process medians; ratios are medians of paired ratios. The
[full report](../../../../docs/validation/2026-09-10/scanner-integration/)
retains all conditions, raw samples, immutable build identities, instruction
counts and the isolated experiments. See [native-summary.json](native-summary.json)
and [python-summary.json](python-summary.json) for complete aggregates.

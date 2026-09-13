# Closing-tag integration benchmarks

Reusing validated opening names reduces native project time by **5.1%** and
actual CPython time by **2.8%** versus the preceding runtime. Every fixed
condition improves against that control. Overall times remain **1.91×** and
**1.38×** Expat respectively.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 65.120 ms | 29.120 ms | 2.23× |
| Wayland protocol | 1.360 ms | 1.073 ms | 1.26× |
| Maven POM | 1.047 ms | 0.439 ms | 2.39× |
| Batik SVG | 0.138 ms | 0.134 ms | 1.04× |
| GTK UI | 0.452 ms | 0.213 ms | 2.11× |
| DocBook XSL | 0.327 ms | 0.185 ms | 1.77× |

The displayed subset uses 4 KiB feeds with namespaces disabled. Times are medians
of process medians; ratios are medians of paired ratios. The
[full report](../../../../docs/validation/2026-09-10/end-tag-integration/)
retains all conditions, raw samples, exact build identities, compatibility
outcomes and the separate isolated study. See [native-summary.json](native-summary.json)
and [python-summary.json](python-summary.json) for complete aggregates.

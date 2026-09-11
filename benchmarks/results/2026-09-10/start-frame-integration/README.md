# Literal start-frame integration benchmarks

Lowering validated literal tags directly into detached callback frames reduces
native project time by **9.3%** and actual CPython time by **4.4%** versus the
preceding runtime. Every fixed condition improves against that control. Overall
times remain **1.75×** and **1.32×** Expat respectively.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 56.530 ms | 28.872 ms | 1.95× |
| Wayland protocol | 1.259 ms | 1.075 ms | 1.17× |
| Maven POM | 0.951 ms | 0.432 ms | 2.21× |
| Batik SVG | 0.138 ms | 0.134 ms | 1.03× |
| GTK UI | 0.417 ms | 0.214 ms | 1.95× |
| DocBook XSL | 0.287 ms | 0.184 ms | 1.56× |

The displayed subset uses 4 KiB feeds with namespaces disabled. Times are medians
of process medians; ratios are medians of paired ratios. The
[full report](../../../../docs/validation/2026-09-10/start-frame-integration/)
retains all conditions, raw samples, exact build identities, compatibility outcomes
and the separate isolated study. See [native-summary.json](native-summary.json)
and [python-summary.json](python-summary.json) for complete aggregates.

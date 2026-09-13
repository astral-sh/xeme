# Bounded text search: real XML benchmarks

The normal release takes **3.3% less native project time** and **2.4% less actual CPython time** than `5bc806e`. It remains **2.04× Expat natively** and **1.45× through CPython**. All 24 conditions per real-project cohort and all generated controls are retained, including regressions.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 67.187 ms | 28.823 ms | 2.33× |
| Wayland protocol | 1.464 ms | 1.066 ms | 1.39× |
| Maven POM | 1.153 ms | 0.442 ms | 2.61× |
| Batik SVG | 0.145 ms | 0.133 ms | 1.08× |
| GTK UI | 0.497 ms | 0.213 ms | 2.33× |
| DocBook XSL | 0.340 ms | 0.186 ms | 1.82× |

These rows use 4 KiB feeds with namespaces disabled. Times are medians of seven process medians; ratios are medians of paired ratios. The [complete report](../../../../docs/validation/2026-09-10/coalesced-search/) includes both feed sizes, native namespace modes, ElementTree and pyexpat, raw samples, exact build identities, source and independent reviews. The corpus is original pinned project XML, without full-project or external-DTD timing. No PGO or allocator substitution is used.

The [preceding frame report](../detached-frames/) retains its separate comparison against `4b11ace`.

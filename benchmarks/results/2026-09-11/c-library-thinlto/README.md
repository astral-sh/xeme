# C library ThinLTO benchmarks

Effective ThinLTO on the `50c20c1` parser reduces elapsed time by 1.8% across 24 native project conditions and 1.1% across 24 actual CPython conditions versus the identical normal build. Oriole still takes 1.73× Expat's time natively and 1.31× through CPython. This does not meet the overall performance goal.

| Project XML | Oriole | Expat | Oriole / Expat |
| --- | ---: | ---: | ---: |
| Vulkan registry | 56.367 ms | 29.392 ms | 1.91× |
| Wayland protocol | 1.260 ms | 1.078 ms | 1.17× |
| Maven POM | 0.955 ms | 0.450 ms | 2.12× |
| Batik SVG | 0.134 ms | 0.135 ms | 0.99× |
| GTK UI | 0.427 ms | 0.214 ms | 1.99× |
| DocBook XSL | 0.291 ms | 0.185 ms | 1.57× |

These six displayed conditions use 4 KiB chunks with namespaces disabled. The fixed suite retains both chunk sizes, both namespace modes, all generated controls and all regressions. Times are medians of process medians; ratios are medians of paired ratios. The sole native Expat win is Batik here, by 0.65% on a shared host.

The [complete report](../../../../docs/validation/2026-09-11/c-library-thinlto/) contains build commands, compiler and library identities, all aggregate results, raw samples, compatibility results, independent reviews and reproduction controllers. The measured libraries do not use PGO.

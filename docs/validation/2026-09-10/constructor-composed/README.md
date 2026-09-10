# Compose the namespace-disabled constructor optimization

The parser initializes the built-in `xml` namespace binding only when namespace expansion is enabled. The empty map retains its selected allocator and randomized hasher. Nonnamespace constructor calls fall from **9 to 6** (from **10 to 7** with an explicit ASCII encoding); namespace-enabled construction is unchanged.

The root release passes **302 Rust checks**, strict Clippy/formatting, **1,518 exact ordinary comparisons**, **6,468 unchanged attribute observations**, **372 custom-attribute conditions**, **384 unchanged malformed-attribute observations**, **2,392 malformed-text cases**, **32 streaming/context/default/resume cases**, and **36,456 custom-encoding comparisons**. All **237 counted public-API cases** finish without live selected allocations and with exact accepted-codec release counts; **144 context conditions** preserve the preceding root behavior.

The original **4,740 upstream API configurations** now report **4,077 passing / 663 failing**, fixing exactly the **12** constructor retry configurations with no regressions. Original assertions, retry ceilings and process bounds are unchanged. Six native C sanitizer suites pass with **327 allocation-failure scenarios per linkage**. C sanitizers cover callers; Rust is uninstrumented and leak sanitizer is disabled. The archive records the correction from an initial packaging assertion that reused the older isolated330-scenario count; the completed root suites and runtime did not change.

The [isolated constructor evidence](../lean-constructor/) retains the source review, original failed local minimum-count assertion, debug timeouts, unsupported reference lifecycle probes, and tiny-input timing cohort. Its local test helper requires a nonempty allocation workload and still sweeps every measured failure index. This composed checkpoint makes no new project throughput, PBS or sustained-fuzz claim.

[report.json](report.json) records exact sources and library identities. [files.json](files.json) verifies [evidence.tar.gz](evidence.tar.gz), including original outcomes, commands and source review.

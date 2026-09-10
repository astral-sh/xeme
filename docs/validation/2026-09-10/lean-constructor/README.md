# Namespace-disabled constructor storage

The patch is based on `7c0d8b35c079c00da3012628ca56d7b35d2c5cd4`. It omits the unused built-in namespace binding when namespace expansion is disabled. The empty map keeps its allocator and salted hasher; explicit child contexts and namespace-enabled parsing keep their behavior.

Selected constructor calls fall from 9 to 6, or 10 to 7 with a requested encoding. The unchanged upstream API suite gains all 12 constructor-test configurations: 4,077 pass / 663 fail, with no new failures, signals, or timeouts in the release comparison. Both native linkage modes pass all 330 dynamically measured allocation-failure scenarios. The 286 core/FFI tests and strict Clippy pass.

The small internal allocation workload now makes 93 calls. Its test helper's arbitrary >100-call minimum was replaced with a nonempty check; every measured failure index remains exercised. The initial failure is preserved. No upstream test ceilings or assertions changed.

The 144 context oracle rows and 108 additional robustness rows have no baseline changes. The latter deliberately include manual contexts/child creation before Expat's documented first-external-callback precondition; Expat aborts in 20 parent-free rows. Those are not conformance or security findings. The original aborted attempt and isolated results remain included.

The initial debug upstream run had two 2 GiB-input timeouts; the final release run uses the parent's exact bounds and restores their existing assertion-failure outcomes. Both runs are preserved.

Seven paired native processes per configuration measured roughly 7–8% lower time on the three tiny nonnamespace inputs. Namespace controls and real-project results were mostly similar. These are parser microbenchmarks, not end-to-end project measurements, and shared-host variation remains. Every timed sample is checked against complete callback preflights and an independent native hash/count calculation.

`summary.json` records exact counts and scope; `builds.json`, `build-source.json`, and `source.tar.gz` identify the source and local builds. No binaries are packaged. `independent-review.json` is the root agent's separate source review. The original upstream failures and all unsuccessful experiments remain included.

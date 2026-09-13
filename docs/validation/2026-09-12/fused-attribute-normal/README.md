# Fused attribute scanning on the current parser

**Experimental; the 10% performance target remains unmet.** Normal campaigns improve 23 of 24 real-input conditions in both native C and CPython: 1.91% and 1.44% less time than the current parser, respectively. Native and Python time remain 51.26% and 21.60% above Expat respectively; four of 24 conditions in each real-input campaign meet the 10% target. This does not establish production readiness.

The candidate fuses ordinary ASCII attribute delimiter scanning with literal-value eligibility. Non-ASCII and special values retain the original fallback and error order. It changes only `tag.rs` on source `4815cf78d7419a18df8d698aaae0bbde8daa526f`, preserving the selected expanded namespace Start and capacity refinement. The measured bytes are committed as `09da8b14f06720f23c526eb091e3f34efeb09630`; [committed-source.json](committed-source.json) verifies every source file against the measured manifest. Independent source review found no actionable issue. Formatting, all-target Clippy and 444 Rust tests pass, with no failed or ignored tests.

## Normal performance

| Consumer | Candidate / current control | Candidate / Expat | Faster than control | Within 10% of Expat |
| --- | ---: | ---: | ---: | ---: |
| All Python | 0.9856× | 1.2160× | 23/24 | 4/24 |
| ElementTree | 0.9861× | 1.2856× | 11/12 | 2/12 |
| pyexpat events | 0.9851× | 1.1501× | 12/12 | 2/12 |
| Native real XML | 0.9809× | 1.5126× | 23/24 | 4/24 |
| Native generated controls | 0.9902× | 4.0306× | 4/4 | 0/4 |

The sole native real-input regression is Maven at 4 KiB, namespaces off: **1.0041× current control**, or 0.41% more time. The sole Python regression is Batik/ElementTree at 4 KiB: **1.0137× current control**, or 1.37% more time. All 52 conditions and both regressions are retained in the evidence CSVs. `README-table.md` and `.json` provide the six native 4 KiB/namespace-off rows, including exact process medians and paired ratios.

Measurements use the same six original project XML inputs, 4/64 KiB chunks and seven seeded pairs on the Linux AMD EPYC-Milan host. These are XML-consumer workloads, not whole-project executions. Both Oriole libraries use the same normal generic x86-64 O3/ThinLTO/one-codegen-unit recipe; Expat retains its normal GCC 13.3 O3 build. There is no compiler-treatment or allocator change.

Unmodified CPython 3.12.13 pyexpat and ElementTree extensions use the same O2 recipe. The two candidate extensions are fresh builds; the four current-control/Expat extensions are reused in their original directories with original source/compiler/log/binary identities pinned. All engines receive fresh preflights. The saved audit verifies all 576 workers and 26,364 samples, module and parser origins, callback outputs, seeded ordering and paired medians. Creation, parsing, callbacks and explicit destruction are timed; imports, input reads and output checking are outside. GC remains enabled.

The native audit reconstructs all 672 workers and 106,176 samples from the unchanged faithful C driver, including all callback hashes/counts, library argv/version checks, ordering and sample medians. Native timing includes parser creation, handler registration, feed, callbacks and destruction. Across both campaigns the packet retains 1,248 workers and 132,540 samples.

## Compatibility and source scope

Fresh candidate checks preserve all **4,740 original API configurations: 4,347 pass, 391 assertion failures and two timeouts**, with no changed rows or relaxed assertions/limits. Six C consumers pass. Those harnesses use ASan/UBSan; the linked Rust normal libraries are uninstrumented and leak detection is disabled.

Shared and static strict CPython runs preserve all **802 distinct method outcomes per linkage**, including the same two callback-grouping failures and raw exit 2. Their 809 rendered outcome lines include additional subtest/class-setup lines. Both existing supplemental semantic tests pass for each linkage. Strict consumers include only the explicit upstream pyexpat cleanup backport, separate from the unmodified timed consumers; no tests or assertions are patched.

The benchmark's canonical outputs coalesce adjacent character callbacks and do not replace these strict outcomes. There is no fresh candidate sustained-fuzzing, installed PBS or CI result in this packet.

Separate completed diagnostics apply to the **selected control source `4815cf78…`, library `a240099f…`**: 72 raised-retry configurations preserve their semantic assertions, and 12 buffer-continuation configurations preserve their original tail while observing the differing allocation result. They do not reclassify the original 4,740 API outcomes or establish exhaustive allocation-failure coverage. These are control diagnostics, not candidate validation.

The four accompanying instruction profiles also use that control library and normal Expat, on Vulkan/Maven with namespaces off and 64 KiB feeds. They collect both warmup and measured `XML_Parse` windows. Their instruction counts motivate further investigation of repeated text scans and event processing; they are not candidate profiles, elapsed savings or an optimization claim.

PR 152's earlier-source rejection remains historical evidence. Its normal CPython arm was never executed; the fresh current-source normal campaign here supplies that missing measurement. The prior source/build treatment is not carried forward as a result for this candidate.

## Evidence

`report.json` retains exact library identities, completed outcomes and paths/hashes of the raw records. Source manifests cover all 73 current files; source/compiler comparison confirms only `tag.rs` changes. Raw worker files, original build/compile logs, source patches, review receipts and all condition rows are retained in the compact evidence packet. Executables, libraries and compiler profile data are excluded.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 3,380 indexed files (6,980,176 compressed bytes). Executables, libraries and compiler profile data are excluded; current-control Callgrind text is retained. [index.json](index.json) records original paths, sizes and extraction hashes; every member and original input was read back after writing. [report.json](report.json) preserves exact results and known limitations.

```console
sha256sum evidence.tar.gz
tar -xzf evidence.tar.gz
```

Archive SHA-256: `4f27eb60b048c86d960da608227f8ce46e2d2e6ecae1e831c55020aa18b63833`. The manifest maps all73 candidate/control source bytes to portable archive members.

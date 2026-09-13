# Successful parsing beyond allocation retry ceilings

Four upstream allocation tests stop at their retry ceilings before reaching later parsing assertions. Separate probes now exercise those success paths with failure injection disabled. **All 96 comparisons pass**, across six chunk widths and two deferral modes, against both accepted Oriole and Expat 2.8.4.

The original suite remains **4,347 passing / 393 failing configurations**. Its raw result hash remains `dcda1affc7cfa69d1e77dd60b7c76360e572e2a7767d6d4ded084ab1575647d5`. These probes add semantic coverage; they do not change allocation schedules, resource limits, version identity, or the original expectations.

| Upstream test | Successful behavior checked |
| --- | --- |
| `test_nsalloc_long_element` | Element namespace triplets in start/end callbacks and an attribute triplet in the start callback |
| `test_alloc_dtd_default_handling` | Exact accumulated Default/text bytes and all 11 original handler flags |
| `test_alloc_attribute_enum_value` | External DTD parsing, attlist callback, and child completion/free |
| `test_nsalloc_long_default_in_ext` | Successful parent and external-child parsing/completion/free with long declared defaults |

The final fixture has no delivered-attribute-value assertion. These probes do not establish allocation-failure cleanup, leak freedom, or the later assertions in other ceiling-limited tests. The early-child OOM test was excluded because removing failure injection would invalidate its expected error.

## Permanent regression

[`namespace_long_names`](../../../../tests/c/integration.c) retains the original long-name XML and namespace expectations. It additionally requires attribute value `12`, no extra attributes, exactly one start/end callback, and balanced custom-allocation counters after every parser. It runs six chunk widths with both deferral settings on Expat 2.6 or later; older headers exercise their default behavior. The existing C CI job runs this consumer against both implementations.

The complete consumer passed against five pinned libraries: normal and PGO Oriole, each shared and static, plus shared PGO Expat 2.8.4. That includes 60 executions of the new namespace case. All parser source and headers match the accepted PR121 runtime; only the C integration test changes.

The first local harness stopped before executing a parser because it expected a SONAME-style `ldd` line; Rust's library appeared as an absolute dependency. The corrected harness verified the exact resolved libraries and completed all five consumers. Its subsequent system-Expat check stopped at missing `expat.pc`, before compilation. Both attempts are retained. CI installs the development package for that reference check.

## Evidence

[`success-probes.tar.gz`](success-probes.tar.gz) contains the original and derivative test bodies, unchanged upstream callback helpers, notices, runners, exact commands and outcomes, and independent reviews. [`permanent-consumer.tar.gz`](permanent-consumer.tar.gz) contains both local harness attempts and the permanent source snapshot. Binaries are identified by hashes; they are not bundled. See [`evidence.json`](evidence.json) for archive identities.

The probes use Expat source commit `12cf0b1f25f026a022fe728ad8f7e3d017285b80`, accepted Oriole source commit `5b9fb0f78a2d3cb9d56350378b23ae996e2164bb`, Oriole PGO library `8cc1c414442727b3649acea7222e7b5827b309f818f302264840d89952f73a4a`, and Expat PGO library `12d33ad26315e8b46a02e598df8581679d0561fb2d98a3d4744e483a573fbdd0`. These are successful-parsing tests, not performance or sanitizer results.

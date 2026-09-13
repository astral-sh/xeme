# Remaining original API failures

Saved-record census of selected reference-frame PGO library `d3c43850247b83aa3089b7f08deb31a084ee2aecb47b52d27e9d05e97b0e1bd4`, measured source `0f66d54ac8418f0a6e628ad18570677a19c9ed45` (source-equivalent restack `8b00d4cbb3f540647e1d7d795d0a04df00ed92f4`). No parser, compiler or test target was executed for this census.

**No new substantive XML/callback defect is established by these remaining failures.** The original matrix remains **4,347 passes, 391 assertion failures and two timeouts** across 4,740 configurations / 395 test bodies. The 393 nonpassing configurations span 38 test names. This classification does not turn any failing result into a pass.

| Reached failure category | Configurations |
| --- | ---: |
| Allocation retry ceilings | 298 |
| Expected realloc schedule | 44 |
| Allocation after empty ParseBuffer | 12 |
| Fixed-budget child creation | 12 |
| **Allocation-related subtotal** | **366** |
| Literal implementation version identity | 12 |
| Fixed single-buffer resource policy | 12 |
| Deferral allocation-growth assertion | 1 |
| Three-second 2 GiB timeout | 2 |

Original bounds remain six chunks (0–5), two deferral settings, three seconds per case, 1 GiB address space, 768 MiB RSS, 240 seconds overall, and the same twelve private-test exclusions. Timeout is not successful completion.

## What the records establish

The raw log and result JSON agree on all 4,740 ordered outcomes. The assertion locations below and their original source bytes match the producer manifest. The allocator-related records identify exhausted budgets or a different allocation schedule; they do not establish that the associated XML construct is unsupported. They also do not establish later assertions that execution did not reach. Exact allocator causes and all later failure ordinals are not inferred from the test name.

`test_misc_version` reaches its final literal comparison with `expat_2.8.4`; numeric and parsed versions already agree. `XML_ExpatVersion` deliberately returns `oriole_compat_2.8.4` (`crates/oriole_expat/src/lib.rs:2673`). `test_buffer_can_grow_to_max` asks for roughly 1 GiB while `XML_GetBuffer` caps one request at 256 MiB (`:1737`, constant at `:39`); the 512 MiB live family limit is independent. Removing identity or resource policies solely to improve the score is not a semantic fix.

The sole deferral failure is `g_totalAlloc - alloc_before < 4096`, not an XML acceptance or callback-value assertion. Its buffer-pressure mechanism and three performance candidates were already investigated in `docs/validation/2026-09-11/c-reservation-experiments/`; they were rejected. The two active `test_misc_input_2gb` contexts end under the unchanged three-second alarm, without a final parser outcome.

The separate current 72-configuration raised-ceiling replay reaches original flag/text oracles in six selected tests. It is supplementary semantic evidence with changed diagnostic budgets; this original matrix remains unchanged. Long inherited default values, successful nested entities and phase-aware nested OOM already have permanent C regressions. Those resolved coverage gaps are not new fix candidates.

## Remaining readiness work

- Preserve this exact census and the separate diagnostic scope in compatibility documentation; avoid implying 391 independent semantic defects or full compatibility.
- Keep the two strict CPython callback-grouping failures explicit for the intended opt-in deployment.
- Complete selected-source distribution/platform validation and sustained adversarial, fuzz and sanitizer coverage before a default PBS substitution; older source-specific reports do not automatically establish the current build.
- Continue measured performance work: PGO native 1.326116× and combined CPython 1.117937× Expat remain outside roughly 1.10×. No new narrow runtime compatibility fix is justified by this census alone.

## Exact original test assertions

Paths below are relative to the `original/` directory inside `records.tar.gz`; they name archive members, not expanded files in this publication directory. For timeouts, the location is the test body, since no source assertion fired. `per-name.csv` additionally records the exact source line; `report.json` retains raw diagnostics and every nonpassing context.

| Original test | Configurations | Category | Assertion / body |
| --- | ---: | --- | --- |
| `test_nsalloc_parse_buffer` | 12 | empty buffer allocation expectation | `adapted/nsalloc_tests.c:129` |
| `test_nsalloc_long_element` | 12 | allocation retry ceiling | `adapted/nsalloc_tests.c:502` |
| `test_nsalloc_realloc_binding_uri` | 12 | reallocation schedule | `adapted/nsalloc_tests.c:541` |
| `test_nsalloc_realloc_long_prefix` | 2 | reallocation schedule | `adapted/nsalloc_tests.c:617` |
| `test_nsalloc_realloc_longer_prefix` | 2 | reallocation schedule | `adapted/nsalloc_tests.c:693` |
| `test_nsalloc_long_context` | 12 | allocation retry ceiling | `adapted/nsalloc_tests.c:934` |
| `test_nsalloc_long_default_in_ext` | 12 | allocation retry ceiling | `adapted/nsalloc_tests.c:1403` |
| `test_nsalloc_long_systemid_in_ext` | 12 | allocation retry ceiling | `adapted/nsalloc_tests.c:1472` |
| `test_nsalloc_prefixed_element` | 12 | allocation retry ceiling | `adapted/nsalloc_tests.c:1508` |
| `test_alloc_run_external_parser` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:347` |
| `test_alloc_external_entity` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:404` |
| `test_alloc_ext_entity_set_encoding` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:432` |
| `test_alloc_internal_entity` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:460` |
| `test_alloc_parameter_entity` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:485` |
| `test_alloc_dtd_default_handling` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:537` |
| `test_alloc_realloc_buffer` | 12 | reallocation schedule | `adapted/alloc_tests.c:609` |
| `test_alloc_ext_entity_realloc_buffer` | 12 | reallocation schedule | `adapted/alloc_tests.c:635` |
| `test_alloc_public_entity_value` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:734` |
| `test_alloc_set_foreign_dtd` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:920` |
| `test_alloc_attribute_enum_value` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:952` |
| `test_alloc_notation` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1173` |
| `test_alloc_public_notation` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1223` |
| `test_alloc_system_notation` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1272` |
| `test_alloc_nested_groups` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1311` |
| `test_alloc_long_attr_default_with_char_ref` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1608` |
| `test_alloc_long_attr_value` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1652` |
| `test_alloc_nested_entities` | 12 | fixed budget child creation | `adapted/handlers.c:496` |
| `test_alloc_realloc_param_entity_newline` | 2 | reallocation schedule | `adapted/alloc_tests.c:1738` |
| `test_alloc_realloc_ce_extends_pe` | 2 | reallocation schedule | `adapted/alloc_tests.c:1784` |
| `test_alloc_long_base` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1912` |
| `test_alloc_long_public_id` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:1959` |
| `test_alloc_long_entity_value` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:2007` |
| `test_alloc_long_notation` | 12 | allocation retry ceiling | `adapted/alloc_tests.c:2076` |
| `test_misc_version` | 12 | literal version identity | `adapted/misc_tests.c:224` |
| `test_misc_input_2gb` | 2 | bounded timeout | `adapted/misc_tests.c:932` |
| `test_buffer_can_grow_to_max` | 12 | single buffer policy | `adapted/basic_tests.c:3456` |
| `test_bypass_heuristic_when_close_to_bufsize` | 1 | deferral allocation growth | `adapted/basic_tests.c:6398` |
| `test_nsalloc_realloc_long_ge_name` | 10 | allocation retry ceiling | `adapted/nsalloc_tests.c:1246` |

`records.tar.gz` retains the original matrix, log, manifest, five referenced adapted sources and their Expat license under `original/`. `records.files.json` lists every archive member, byte length and SHA-256, together with the archive hash. All nine members were read back and matched their source bytes before publication. No expanded source or log tree is vendored here.

`read_census.py` is the unchanged historical data-only summarizer. Its absolute input/output paths identify the original measuring machine; it expects the original API files and library for hash checking and is not a portable archive reader. It does not compile, import or execute a parser. `report.json` likewise preserves original source paths and hashes. Original producer receipts remain authoritative; no repository source or upstream assertion was edited.

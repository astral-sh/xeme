# Nested parameter-entity success coverage

The C integration consumer now includes the successful expansion path from Expat 2.8.4's `test_alloc_nested_entities`. All five pinned permanent consumers passed on their first attempt, covering 60 chunk/deferral configurations. The preceding standalone probe also passed all 24 workers; its original source and results remain intact.

## Permanent regression

[`nested_entities` in integration.c](../../../../tests/c/integration.c) preserves the exact 58-byte root document and 1,090-byte external DTD. The root creates one external child, which declares `pe1`, then `pe2` from `%pe1;`, then `pe3` from `%pe2;`. Each declaration must contain exactly 1,024 bytes: 64 repetitions of `ABCDEFGHIJKLMNOP`, checked byte by byte.

The test checks inherited callbacks, one child creation/completion/free, one `doc` start/end with no attributes, successful root and child completion, final byte/line/column positions, and balanced use of the existing custom allocator. Both parsers receive the same chunk setting, from whole-input mode (`0`) through 1–5-byte chunks. Headers at least 2.6 exercise both reparse-deferral settings; older headers use their default mode, following the existing namespace fixture.

This change adds a C test and its [upstream attribution](../../../../tests/c/UPSTREAM-NOTICES.txt). Parser runtime, headers, Cargo inputs and CI configuration are unchanged from PR124 head `4d603c7487ca93cc73dbd4acfe6fa5714ffc2151`.

## Permanent consumer validation

The full integration consumer passed against the unchanged current Oriole normal and PGO libraries under both shared and static linkage, and against the pinned Expat 2.8.4 PGO shared library. Each consumer ran all 12 nested-entity configurations with assertions enabled, including allocator balance; the existing integration tests ran in the same process. All five compiles, linkage checks and executions passed once, with no retries. Shared-library paths were checked, and static consumers had no dynamic Oriole or Expat dependency.

The [permanent-consumer summary](permanent-consumer.json) records all library and executable hashes. A separate [compact artifact](permanent-consumer.tar.gz) retains the reviewed source, exact controller/delta and every first command's stdout, stderr and outcome; its [member index](permanent-consumer-members.json.gz) records original paths and hashes. Limits remain 60 seconds per compile and 10 seconds each for linkage and execution. No Rust libraries were rebuilt. The optional system `pkg-config` probe was not part of this local gate; unchanged CI exercises its installed reference version. This execution summary was written by the collector after the parent agent's source review.

## Completed standalone results

The first and only compile succeeded. All 24 workers passed: current streaming Oriole PGO and Expat 2.8.4 PGO, each at six chunk sizes and two deferral settings. All 12 paired comparisons agreed on declaration contents, callbacks, child lifecycle and final positions. Every worker ended with zero tracked live bytes and blocks and no failed allocations.

| Observation | Oriole PGO | Expat PGO |
| --- | --- | --- |
| Declaration byte indices | 0, 1043, 1067 | 15, 1058, 1082 |
| Declaration columns | 0, 0, 0 | 15, 15, 15 |
| Final root byte / line / column | 58 / 2 / 7 | 58 / 2 / 7 |
| Final child byte / line / column | 1090 / 3 / 23 | 1090 / 3 / 23 |

Declaration callback positions differ in every pair. They were observational in the standalone protocol and are **not asserted by the permanent test**. The original twelve allocation-injection failures and full 4,740-case matrix remain unchanged. These successful parses do not establish injected-failure cleanup, sanitizer coverage, RSS bounds or complete API compatibility.

## Retained evidence

[Evidence summary](evidence.json) pins both libraries, the runtime source and the first results. The [38,957-byte archive](standalone-evidence.tar.gz) retains all 70 standalone text/source/raw-log files, including exact upstream extracts, original failure rows, preparation records and all 24 first worker outputs. Its SHA256 is `b41de9d4ffe7c12d82955e420e4ef37f0750ed29eae67fa2777b1ce926e36b16`. The compiled probe is excluded with its hash recorded. The [compressed member index](archive-members.json.gz) maps every member to its original file and hash; all members and origins were read back.

The [independent source review](independent-source-review.json) preceded execution; the [independent result review](independent-result-review.json) checked the saved outcomes. These reviews cover the standalone fixture. Its archived preparation-time README is preserved alongside the later `RESULTS.md` and raw results. The summary and permanent C adaptation were written by the standalone collector.

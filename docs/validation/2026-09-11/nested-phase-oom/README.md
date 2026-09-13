# Nested external-entity allocation failure

The new `nested_entities_phase_oom` C regression reaches two successful nested
parameter-entity declarations, then fails subsequent allocations. Both Oriole and
Expat return `XML_ERROR_NO_MEMORY` from the child and
`XML_ERROR_EXTERNAL_ENTITY_HANDLING` from the parent, free the child exactly once,
and release every tracked allocation. No parser implementation changes were needed.

The test reuses the exact XML and successful `pe1`/`pe2` declaration oracle from
Expat 2.8.4's `test_alloc_nested_entities`. Injection starts in the `pe2` callback,
after successful child creation, inside one complete child feed. It remains enabled
through both parser frees. Later partial declaration callbacks are outside this
error-propagation oracle; the existing positive fixture checks all three complete
declarations.

## Results

| Frozen library | Linkage | Result |
| --- | --- | --- |
| Expat 2.8.4, PGO | Shared | Passed |
| Selected Context runtime, normal | Shared and static | Both passed |
| Selected Context runtime, original generated-only PGO | Shared and static | Both passed |

All five builds and five runs passed on their first attempts on CPU 3. Each run
executed the complete existing C integration consumer plus this one added case,
verified all 34 used API symbol origins, observed exactly one rejected allocation
after `pe2`, and ended with zero live tracked allocations. The regression does not
depend on either parser's global allocation ordinal, retry ceilings, or callback
positions. The original upstream 4,740-configuration record remains unchanged:
4,347 passes, 391 assertions, and two timeouts. This closes a separately tested
failure-propagation gap; it does not turn those original failures into passes.

The source base is PR132 `ef0eac091999c2e29e888918ac958480dbdacb4c`.
Frozen selected runtime source manifest:
`8a7da2759b67ca85f82fb29bd775392e532b3ff355340eb10f92ea7764bbe642`.
The namespace-guard optimization is excluded. Full library hashes and commands
are in [results.json](results.json); raw streams are in [logs](logs).

## Validation scope and reproduction

C consumers were compiled with warnings as errors, AddressSanitizer and
UndefinedBehaviorSanitizer. The frozen Rust libraries and reference Expat library
were not sanitizer-instrumented. `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1`
and `UBSAN_OPTIONS=halt_on_error=1` match the preceding C gate. Explicit custom
allocation accounting checks cleanup independently of the disabled leak detector.
Each compile and run had a 120-second wall-clock bound. No Rust rebuild, new full
API matrix, CPython run, benchmark, or distribution build was performed.

[run_checks.py](run_checks.py), [consumer.c](consumer.c), the [source](source)
snapshot, and [checks-preparation.json](checks-preparation.json) preserve the
executed controller and inputs. [readback.json](readback.json) records the
collector's log/hash checks; the [independent root review](root-readback.json)
also passed. [files.json](files.json) indexes the compact packet.
Binaries and Cargo targets are excluded; their identities remain in the report.
The [upstream notice](source/UPSTREAM-NOTICES.txt) accompanies the reused fixture.

To repeat all five consumers using the exact recorded library artifacts:

```sh
taskset -c 3 python3 -I -S rerun.py /tmp/oriole-nested-phase-oom-recheck
```

On another machine, supply a JSON mapping from the five arm names in
`checks-preparation.json` to local library paths as the second argument. Their
hashes must match. The output directory must be new. The helper copies the exact
executed runner and source bytes, changes only path bindings, and retains all
first results. Within the normal repository CI, the regression runs automatically
as part of the existing reference and Oriole C integration consumers.

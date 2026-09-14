# Upstream Expat API tests

`run.py` builds Expat 2.8.4's API tests against a supplied shared library and
records passes, assertion failures, signals, and timeouts. By default, test bodies
are unchanged; the allocation diagnostic below adapts them.

```sh
python3 tools/upstream-expat/run.py \
  --source /path/to/expat-2.8.4 \
  --config /path/to/expat-build/expat_config.h \
  --library /path/to/libxeme_expat.so \
  --output /tmp/xeme-upstream-api
```

The upstream checkout must be clean at commit
`12cf0b1f25f026a022fe728ad8f7e3d017285b80`. Use the same generated narrow-character
configuration for the reference and candidate runs. By default the runner
exercises chunk sizes 0 through 5 with reparse deferral disabled and enabled.
Use `--chunks 0 --deferral 1` for a shorter run, or
`--tests test_name other_test_name` to select individual tests. Missing selected
test results fail the run.

The output directory must be empty.

## How the adapter works

Expat's normal CMake test target compiles its own parser sources directly. This
adapter compiles only test sources, links the selected library, and verifies
the loaded `XML_Parse` origin. The manifest records hashes of the library, adapter,
generated sources, configuration, and executable, plus the compiler command.

The test framework normally uses `longjmp` on assertion failure. A failed
assertion inside a callback would jump over live Rust frames and their cleanup.
Each test therefore runs in a separate child process, including setup and
teardown; assertion failure exits that child immediately. The parent records
the outcome and continues. Children receive a configurable address-space ceiling
(1 GiB by default), a polled resident-memory ceiling (768 MiB by default), and a
timeout. The entire run has a deadline too. Resident-memory checks use Linux
`/proc`; hitting a resource limit is reported as a failure.

Twelve named tests of private Expat implementation details are excluded. Their
exact names and reasons appear in `manifest.json` and `results.json`: SipHash,
private UTF-8 helpers, private allocation functions, and internal accounting or
scanning counters. Reparse defaults are applied through public constructor/reset
setters instead of Expat's private global setting.

The adapter applies [memcheck.patch](memcheck.patch) to Expat's test-only
tracking allocator. The original tail-removal path stores `entry->next` (always null for
the tail) instead of `entry->prev`, so a later allocation crashes when earlier
allocations remain. [allocator_repro.c](allocator_repro.c) reproduces this with
four allocator calls and no XML parser linked. The patch leaves test assertions
unchanged.

`tests.log` retains assertion diagnostics and per-test outcomes. `results.json`
contains structured results and the test process's exit code. The runner exits
nonzero if tests fail.

## Allocation behavior

`--allocation-behavior` adapts the tests to allow Xeme's allocation strategy. It
covers all 83 public allocation-suite tests, including
`test_mem_api_cycle` and `test_mem_api_unlimited`, plus
`test_bypass_heuristic_when_close_to_bufsize`. The default 12 configurations
produce 1,008 reported outcomes per engine. The deferral test runs 504 size
combinations with whole-buffer input and deferral enabled; its other 11
configurations return early as in upstream.

```sh
python3 tools/upstream-expat/run.py \
  --source /path/to/expat-2.8.4 \
  --config /path/to/expat-build/expat_config.h \
  --library /path/to/libxeme_expat.so \
  --allocation-behavior --output /tmp/xeme-allocation-behavior
```

The [adapter](allocation_behavior.py) checks source hashes and the test inventory
before changing copied sources. It raises retry ceilings to 512,
permits success without Expat's allocation counts or buffer-growth schedule, and
sweeps two previously fixed allocation-failure points. It retains checks for
callback data, handler flags, parser states, error propagation and eventual
success. An empty parse may succeed without allocating; if it fails there, it
must report `XML_ERROR_NO_MEMORY`.

The [allocation tracker](allocation_tracker.c) records the injected allocator's
successful allocations, reallocations, frees and denied requests. Teardown
checks detect retained blocks; unknown or repeated frees, invalid reallocations
and unexpected system allocation failures fail the run. Counts describe
allocator calls across retries and configurations, not distinct failure sites.
Tracking covers allocations through the test memory suites. This diagnostic
also tracks default-constructor allocations without injecting failures, so the
public memory-API and allocation-setting tests receive the same checks.

`allocation-behavior.patch` records the source changes. The manifest records hashes
of the sources, adapter, tracker, patch, library, configuration, and executable.
`tests.log` includes an ownership report for each completed test.

CI uses `tools/compatibility.py allocation` to run both engines. Every test,
retained assertion, and ownership check must pass. Use `--tests` for focused local
runs; CI requires the complete inventory.

The original API mode has its own failure baseline. Some allocation tests assert
successful parsing without checking complete event data, and the bounded sweeps
do not cover every possible allocation failure. See the
[test results](../../docs/evidence/2026-09-13-allocation-behavior.md).

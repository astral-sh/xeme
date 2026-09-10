# Upstream Expat API tests

`run.py` builds the pinned Expat 2.8.4 test sources against a supplied shared
library. It preserves public test bodies and records every pass, assertion
failure, signal, and timeout. Failures remain failures; unsupported Oriole
capabilities receive no waivers.

```sh
python3 tools/upstream-expat/run.py \
  --source /path/to/expat-2.8.4 \
  --config /path/to/expat-build/expat_config.h \
  --library /path/to/liboriole_expat.so \
  --output /tmp/oriole-upstream-api
```

The upstream checkout must be clean at commit
`12cf0b1f25f026a022fe728ad8f7e3d017285b80`. Use the same generated narrow-character
configuration for the reference and candidate runs. By default the runner
exercises chunk sizes 0 through 5 with reparse deferral disabled and enabled.
`--chunks 0 --deferral 1` selects one explicitly reported context for a shorter
initial run. `--tests test_name other_test_name` selects exact public test names;
the manifest records that selection, and missing selected test results fail the run.

The output directory must be empty so reruns cannot overwrite earlier evidence.

## Adapter boundaries

Expat's normal CMake test target compiles its own parser sources directly. This
adapter compiles only test sources, links the selected library, and verifies
the loaded `XML_Parse` origin. Library, adapter, generated-source, configuration,
and executable hashes are recorded alongside the exact compiler command.

The test framework normally uses `longjmp` on assertion failure. A failed
assertion inside a callback would jump over live Rust frames and their cleanup.
Each test therefore runs in a separate child process, including setup and
teardown; assertion failure exits that child immediately. The parent records
the outcome and continues. Children receive a configurable address-space ceiling
(1 GiB by default), a polled resident-memory ceiling (768 MiB by default), and a
timeout; the entire run is also time bounded. The adapter currently uses Linux
`/proc` for resident-memory observation. These process
bounds are recorded and can cause failures independently of XML compatibility.

Twelve named tests of private Expat implementation details are excluded. Their
exact names and reasons appear in `manifest.json` and `results.json`: SipHash,
private UTF-8 helpers, private allocation functions, and internal accounting or
scanning counters. No fake implementation counters are supplied. Reparse
defaults are applied through public constructor/reset setters instead of
Expat's private global setting.

The adapter also applies [memcheck.patch](memcheck.patch) to Expat's test-only
tracking allocator. Its tail-removal path stores `entry->next` (always null for
the tail) instead of `entry->prev`, so a later allocation crashes when earlier
allocations remain. [allocator_repro.c](allocator_repro.c) reproduces this with
four allocator calls and no XML parser linked. The correction preserves every
test assertion; the original ASan crash and corrected run are recorded separately.

`tests.log` retains assertion diagnostics and per-test outcomes. `results.json`
contains structured results; a nonzero exit remains nonzero. This is an adapted
public API suite, not a claim that Expat's internal test suite passes unchanged.

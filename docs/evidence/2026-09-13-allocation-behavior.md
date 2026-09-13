# Allocation behavior after relaxing allocation-count assumptions

Both Xeme and pinned Expat passed all **1,008 reported test/configuration
outcomes** in the separate allocation-behavior diagnostic. Every retained
assertion and custom-allocator ownership check passed. This exercise found no
additional runtime failure requiring a fix. The original API mode still reports
509 known Xeme failures; adapted passes do not erase them.

## Tested source

The Xeme runtime is commit `50ca961b4312b3a2e1ee500891220b44dd552cee`, with no
runtime edits for this diagnostic. A release build used
`cargo +ohm rustc --release -p xeme_expat --lib --crate-type cdylib,staticlib`
and Ohm Rust `1.98.1-dev` (`f6270311094cd4b48fefce03debdffcf8396c64c`) on
`x86_64-unknown-linux-gnu`. The build manifest records source hashes before and
after compilation and confirms they were unchanged.

| Input | SHA-256 or revision |
| --- | --- |
| Tested Xeme shared library | `be9ef7dd08c9f0fe26cc7fc102c674919fecc30893a539a065b6690d28550b44` |
| Expat 2.8.4 source | `12cf0b1f25f026a022fe728ad8f7e3d017285b80` |
| Expat shared library | `7a333bc8ac92ff53b8fc1ecc68711802e7c5781f52d3d0592a6f0ee362d9f478` |
| Shared narrow-character configuration | `ec397da0c61b6bab207c7aba9f830b5e69f79c8462e58d97c6e6891c87c44a4d` |
| Build manifest | `b44f786d361a187b75f26348653696223b487849225451fd206c3b3891f3c8ef` |

The test executables used `cc -O1` and the same adapted upstream sources. Each
test had a three-second timeout, 1 GiB address-space limit and 768 MiB RSS limit;
each complete engine run had a 240-second timeout. Loaded library origins were
verified. The test adapters were working-tree changes identified by hashes in
the manifests, separate from the unchanged runtime commit.

## Scope and retained checks

The selection contains 81 public `test_alloc_*` / `test_nsalloc_*` tests, the two
`test_mem_api_*` tests, and the deferral-growth test. Each appears in all six
chunk-size configurations with deferral disabled and enabled. The deferral test
executes 504 combinations of leading text, token size and buffer fill in its one
active configuration; its other 11 configurations return early as in upstream.

The [adapter](../../tools/upstream-expat/allocation_behavior.py) records its
copied-source changes and rejects drift in the pinned input hashes or inventory:

- Retry ceilings rise to 512. Tests still require eventual success; they no
  longer require a particular minimum or exact allocation count.
- A nested-entity fixture sweeps child parsing after successful parser creation.
  Failed child parses must report `NO_MEMORY`, the parent must report
  `EXTERNAL_ENTITY_HANDLING`, and sufficient-budget parsing must succeed.
- A child-creation fixture sweeps allocation budgets and checks parent reset
  after each attempt. The general upstream expected-error handler remains strict.
- Empty buffer parsing may succeed without allocating. Its failure alternative
  must be `NO_MEMORY`; later suspension, resumption and finished-parser checks
  remain active.
- The deferral test retains parse statuses, bounded progress and final element
  count while dropping Expat-specific allocation-size and growth assertions.
- Existing callback data, namespace triplet comparisons and handler flags remain
  active. Custom memory suites additionally check allocation ownership at teardown.
- Diagnostic default constructors use a tracked, non-injecting memory suite,
  covering the public memory-API and allocation-setting fixtures as well. The
  original API mode keeps its normal constructor calls.

## Results

| Engine | Reported passes | Denied malloc calls | Denied realloc calls | Live tracked blocks at completion | Unexpected system allocation failures |
| --- | ---: | ---: | ---: | ---: | ---: |
| Xeme | 1,008 / 1,008 | 24,127 | 1,721 | 0 | 0 |
| Expat | 1,008 / 1,008 | 10,915 | 934 | 0 | 0 |

Both result sets passed the strict allocation gate with complete inventories
and one ownership report per test/configuration. The denied-call totals include
repeated requests across retry budgets, fixtures and configurations. They are
**not counts of distinct allocation sites**.

These are adapted upstream tests. Some fixtures only assert successful parsing,
or a subset of callback behavior, rather than complete event equivalence.
Ownership tracking covers the custom allocator suites; this was not a sanitizer
run or an exhaustive check of every process allocation. The retry bounds and
fixtures limit the fault-injection coverage. Remaining callback, position,
resource-policy and W3C differences are described in the
[compatibility guide](../compatibility.md).

## Local artifacts and reproduction

Artifacts are retained locally under `/tmp/xeme-c037-allocation-behavior/`:
`baseline/` contains the build manifest and frozen runtime;
`final-allocation/xeme/` and `final-allocation/reference/` contain copied
libraries, generated sources, manifests, executables, raw logs and results.
`final-allocation/gate.json` records the strict two-engine gate, including the
default-constructor ownership checks. Earlier `trial1-*` directories predate
those extra ownership checks and are not the basis for these results.
`original-api/` separately records 4,740 outcomes per engine: 509 known Xeme
failures, none for Expat, and no regressions or improvements. These local paths
are not a durable release archive.

| Artifact | SHA-256 |
| --- | --- |
| `final-allocation/gate.json` | `cec14fe68ea8242df502da1100c600022804b79d100f8163e1f4cccf3c9c7426` |
| `final-allocation/xeme/manifest.json` | `73b02e4c9e44636683825df1dfc826302097d1179c1d94296a4ef9a7a194eac6` |
| `final-allocation/reference/manifest.json` | `ac46e0035c5ab60af090e892ecc686054558961a6216bdb4a84e9046b58db744` |
| Either `results.json` | `470f0c97b0bdd011ac5a278969edfa66e2933ffeb93efccfca08f4f5dba342f3` |
| `final-allocation/xeme/tests.log` | `c3efe4bbf83a3cbc44038ce20da56c4816a270be0b013891b4b5eab87308a288` |
| `final-allocation/reference/tests.log` | `5b81ec2ab36ac153493a05810e23e2e73113f77786f27bcf7f44fe8b9e5b5771` |
| Either `allocation-behavior.patch` | `bc5bfb1907a7a9182d6da34081aa07ca17e9d2c8018468f1c6e0fdc0ef8395a5` |
| `original-api/gate.json` | `e4be251c78a862a9bf023f78efed8c4c51d89b171d73bff45a83ab537910e7cd` |

Run the full two-engine check in a fresh output directory:

```sh
python3 tools/compatibility.py allocation \
  --library /absolute/libxeme_expat.so --reference /absolute/libexpat.so \
  --source /absolute/expat-2.8.4 \
  --config /absolute/expat-build/expat_config.h --output /tmp/xeme-allocation
```

The [runner guide](../../tools/upstream-expat/README.md#allocation-behavior)
describes focused diagnosis and the recorded adapter boundaries.

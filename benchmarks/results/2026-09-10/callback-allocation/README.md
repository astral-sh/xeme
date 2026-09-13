# C callback allocation

Owned nonempty strings retain one spare byte for a C terminator. Start-element
callbacks use a local pointer array for up to eight attributes and fallible
allocator-owned storage beyond that threshold. Names and attributes remain owned
for the complete callback. Empty Rust strings remain allocation-free.

The isolated comparison uses the preceding multibyte source manifest `718053d9`,
not the newer combined runtime. Three randomized paired processes each take ten
measurements after one warmup, pinned to CPU 3 on the shared host. Exact builds,
inputs, observations, output digests, and allocation counts are retained.

| Workload | Chunk | Baseline | Candidate | Median paired speedup |
|---|---:|---:|---:|---:|
| Elements | 4 KiB | 23.457 ms | 20.668 ms | 1.126× |
| Elements | Whole input | 23.343 ms | 20.820 ms | 1.121× |
| Empty elements | 4 KiB | 10.228 ms | 9.563 ms | 1.078× |
| References | 4 KiB | 25.916 ms | 25.519 ms | 1.016× |

The small reference-workload difference is within the range where shared-host noise
can dominate. The element workload drops from 120,020 malloc and 60,003 realloc
calls to 110,019 malloc and one realloc. The ownership model and family budgets
are retained; the change removes avoidable callback representation work.

Separate direct-Rust and tracked-allocation measurements identify that most remaining
reference-heavy time is in the core parser. Allocation tracking alone accounts for
only a small part of that workload's cost. Hardware profiling is unavailable on
the host; these conclusions come from scoped timing and allocation measurements.

The isolated prototype passes 359 native allocation-failure scenarios and 60 exact
attribute-threshold/Unicode/empty-value comparisons. The combined implementation
passes all 166 workspace tests, strict Clippy, and both shared and static CPython
3.12.13 XML consumers (803 tests and 31 skips each) with the explicit cleanup
backport. Its source and library hashes are in the combined validation archive.
The combined implementation also includes the newer diagnostic and parameter-value
layers; its consumer results are distinguished from the older timed prototype.

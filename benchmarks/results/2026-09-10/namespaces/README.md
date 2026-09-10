# Explicit namespace-mode measurements

This benchmark identifies the combined parameter-value/prolog runtime at `a276f7f`,
before the later C callback allocation optimizations. The native harness enables
namespace expansion in both its full normalized-callback preflight and timed parser
when requested. The same generated inputs are measured in separate disabled and
enabled campaigns; neither campaign stands in for CPython application timings.

Seven randomized pairs of processes each perform ten measured parses after one
warmup. Runs are pinned to CPU 0 on the recorded shared AMD EPYC host. Frequency,
host load, and memory bandwidth remain uncontrolled. Rust release builds use thin
LTO and the recorded Ohm toolchain. All preflights pass and all observed files and
binaries retain their hashes throughout each campaign.

4 KiB input chunks, process-median time per complete parse:

| Namespace expansion | Workload | Oriole | Expat | Paired Oriole / Expat |
|---|---|---:|---:|---:|
| Disabled | elements | 22.226 ms | 3.598 ms | 6.18× |
| Disabled | text | 10.456 ms | 1.614 ms | 6.46× |
| Disabled | entities | 27.109 ms | 2.158 ms | 12.57× |
| Disabled | Prefixed names | 25.811 ms | 3.216 ms | 7.92× |
| Enabled | elements | 28.990 ms | 3.878 ms | 7.53× |
| Enabled | text | 10.681 ms | 1.603 ms | 6.69× |
| Enabled | entities | 27.192 ms | 2.153 ms | 12.59× |
| Enabled | Prefixed names | 30.858 ms | 4.350 ms | 6.88× |

The paired ratio is the median of within-pair ratios, so it can differ from the
ratio of the two overall time medians. Both reports also retain 64-byte and
whole-input measurements, every observation, exact workload bytes, callback
preflight hashes, build manifests, compiler details, and command arguments.

Oriole remains slower than Expat in both modes. These results establish a measured
baseline for subsequent storage optimizations, not a performance or compatibility
acceptance claim.

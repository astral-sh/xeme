# CDATA terminator prefilter: unselected

Searching first for `]` before searching for `]]>` did not provide a useful real-project improvement. PGO was effectively flat; the normal build and generated workloads regressed. The selected Context runtime remains unchanged.

| Build | Real ratio vs control | Adverse real conditions | Generated ratio vs control | Adverse generated conditions |
|---|---:|---:|---:|---:|
| Normal | 1.008904 | 21/24 | 1.024226 | 3/4 |
| PGO | 0.999333 | 14/24 | 1.022407 | 2/4 |

The PGO candidate/Expat ratio was **1.367466** in this campaign. All 56 condition results and all three paired ratios appear in `conditions.csv`; every raw sample and seven-pair result is retained. Separate aggregate ratios need not multiply exactly.

## Change and validation

The candidate replaces the ordinary-text terminator search with a one-byte `memchr` prefilter, followed by the existing cached Finder on the suffix beginning at the first bracket. A prefix without `]` cannot contain `]]>`. The helper restores the original offset and leaves invalid-character precedence, input consumption, raw storage, callbacks and accounting unchanged. No new allocation, unsafe code or parser state is introduced.

Two inline tests compare the helper with an independent three-byte-window oracle: 335,923 short strings, plus UTF-8 prefixes, overlapping brackets and long boundary cases. All 439 local tests, formatting and strict all-target Clippy passed. Fresh normal/generate/use builds passed; the nine actual compiler vectors match the selected controls under the established private-path normalization. All 864 generated training records match. Training data, allocator implementation and compiler settings are unchanged.

The uncommitted candidate is based on `ef0eac091999c2e29e888918ac958480dbdacb4c`, with only `crates/oriole/src/lib.rs` changed; source manifest `0a41677d…` and the full patch/archive preserve it. The selected control is Context runtime `0f28139f…`, source manifest `8a7da275…`, normal library `eed1ee24…` and PGO library `7cd7c5a8…`. Candidate normal and PGO libraries are `109fc6f7…` and `1e8680ff…`.

## Measurement and review

Each mode uses the unchanged native callback driver, 24 real conditions (six projects, namespace off/on, 4 KiB/64 KiB feeds), four generated controls, and seven shuffled pairs with seed 2026091003. Across both modes there are 168 preflight workers and 1,176 timed workers: 210,840 measured samples, 1,176 timed warmups and 336 preflight samples. CPU 0 timing was sequential and isolated from builds and profiling.

Root adapted and executed the build controllers and collected elapsed results. A separate agent authored the source patch; root reviewed it. `next_hotspot` prepared the native adapters and independently reconstructed the saved compiler, training and benchmark results with separate readers. That reviewer also authored the earlier selected Context build controller; this does not constitute independent authorship of every inherited controller. All callback observations, sample medians, shuffled ordering, source hashes and paired arithmetic passed review.

No fresh Python timing or full API, sanitizer, Miri or distribution campaign was run for this rejected candidate. Additional adversarial stress preparation remained disabled. Instruction counts motivated the experiment but do not explain these small elapsed differences.

## Portable evidence

`evidence.tar.gz` retains exact source snapshots, the patch, build helpers, compiler and training logs, local checks, current controller/audit attempts, full native protocols/results, and comparison fixtures with licenses and notices. Binaries are omitted with their identities; build caches and redundant earlier archives are excluded. The 1,344 worker files were byte-compared with reconstruction from the original result containers before omission; `worker-aliases.json` records those exact aliases.

From a relocated package, run `python3 -I -S verify.py --package . --output readback.json`. This checks archive contents, the 72 source files and worker aliases, then reconstructs all native results using saved records only. It loads no parser or compiler. The original local audits separately verified live tool and library identities.

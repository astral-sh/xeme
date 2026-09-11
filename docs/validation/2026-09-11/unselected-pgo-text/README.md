# Unselected PGO training and deferred Text experiments

**Keep the selected PR128 runtime and original generated-only training.**
These three completed experiments did not justify a runtime or recipe change.
Selected source: `78c748d9de5c497858458cbf7a1229148781b2bf`.

| Experiment | Native real treatment/control | Python treatment/control | Decision |
|---|---:|---:|---|
| Add six real files to generated PGO (G+R/G) | 1.006679 | Not run | Rejected: regression |
| Bulk training policy (B/G) | 0.990320 | 0.998939 | Not adopted: modest gains and DocBook regression |
| Deferred C Text raw copy, normal | 0.962975 | Not run | PGO result determines rejection |
| Deferred C Text raw copy, fresh PGO | 1.012515 | Not run | Rejected: regression |

Ratios are geometric means of per-condition median paired ratios; lower is
faster. The four native campaigns each contain 24 real and four generated
conditions. The B Python campaign contains 24 real conditions. All **136
condition rows**, every comparison and all **54 adverse primary comparisons**
are retained in [summary.json](summary.json) and [conditions.csv](conditions.csv).
No results from separate cohorts are combined into a speedup estimate.

## Experiments and limits

**Additive real training:** the original G schedule remains 288 parse records.
G+R appends 48 records: six independently pinned XML files, two feed widths,
namespace off/on and minimal/full handlers. Source, compiler settings and
allocators stay fixed. Both Oriole and Expat receive fresh profiles. Oriole's
real aggregate regressed 0.67% and all four generated conditions regressed
(aggregate 2.19%). No Python or full compatibility gate followed rejection.

**Bulk policy:** B removes only the smallest generated feeds: one byte for
Latin1 and 17 bytes for the other cases, removing 96 parses and 867,464 feed
calls. It retains 192 generated records plus the same 48 real records, 240
total. Its G controls are the frozen
fresh G libraries from the additive study. Oriole improved 0.97% native and
0.11% Python, while Expat improved 4.15% native and 1.69% Python. Matched
Oriole/Expat ratios were 1.43905 native B/B and 1.15577 Python B/B. Ten Python
conditions regressed for Oriole B/G, including DocBook ElementTree at 64 KiB
(+3.22%) and 4 KiB (+2.21%). The recipe was not adopted; no new full
compatibility claim follows these canonical fixture checks.

**Deferred Text raw copy:** the rejected patch deferred only ordinary native
root Text raw bytes to the retained C input context, preserving eager reserve
and failure order. Other paths and ordinary Rust access stayed eager or failed
closed under the C-only protocol. The optional range added 16 bytes to Parser
(2,408 to 2,424). Static excerpts confirm the ordinary C raw copy was removed
and checked materialization moved before subsequent feed/context drain.
Focused checks passed 226 tests plus formatting and strict core/C Clippy.
The first new test failed because its newline-boundary oracle was wrong; its
failure and the test/comment-only correction remain retained. Normal native
improved all 24 real conditions, but PGO regressed 17/24 and all four generated
conditions. The source change was rejected. Python preparation stopped before
any consumer build or run. No full API/ASan or Python result is transferred.

The selected source/header/test map has 70 files. Additive and B share those
bytes; their external training scripts and original G scripts are retained
separately. Text retains both source attempts, the seven-file patch, exact
test/comment correction, source archive and 85-entry build freeze. Its nine
actual workspace compiler vectors and 864 original generated training records
passed the existing matched-build proof. All Oriole builds use O3 ThinLTO with
one codegen unit and explicit-host settings; Expat uses GCC13.3 O3 without LTO.
The exact raw vectors, tool identities, profile files, replay records, warnings
and independent build audits are retained. No compiler-option or allocator
change is inferred from these results.

## Measurement and complete evidence

Native: seven seeded paired cohorts per condition, namespace off/on, 4/64 KiB
feeds, creation/feed/callback/free lifecycle. The four-engine additive and B
campaigns each retain 896 workers and 141,568 samples. Each three-engine Text
mode retains 672 workers and 106,176 samples. Preflights used CPU5; elapsed
workers used reserved CPU0. Worker/aggregate bounds remain 90/600/1,200 seconds.
Native libraries and the driver are pinned by path/hash; this legacy driver
does not attest same-process `XML_Parse` origin.

Python B: unmodified CPython3.12.13 `pyexpat.c` (`fd679a16…`) and
`_elementtree.c` (`de4c3c87…`), eight identical-O2 extension compiler vectors,
96 preflights and 672 timed workers. Its 35,152 samples comprise 34,384 measured
parses, 672 timed-worker warmups and 96 preflight samples. Every worker verifies extension and
`XML_Parse` origins. Both consumers enable namespaces. Construction/feed/close,
Python callbacks/tree creation and explicit destruction are timed; imports,
input reads, canonical checks and explicit `gc.collect` are outside. Automatic
GC remains enabled. Seed, seven-pair shuffle and 300/600/1,200-second bounds
match the original Finder Python protocol. The strict cleanup-backport
consumer is excluded. Coalesced canonical callbacks are a distinct check from
strict compatibility.

The package retains **3,904 workers and 530,640 raw samples**. Original reports
and first failures are unchanged. Each of 3,136 native `worker-*.json` files
can be reconstructed byte-for-byte from its retained preflight/results
container using the explicit inventory alias. All Python compressed stdout,
stderr and spec files are retained. Other identical payloads are stored once
with logical-path aliases. No target/object tree, parser library, extension or
compiler executable is copied. Their recorded hashes remain in the original
reports and exclusion inventory. Full Text assembly/DWARF is excluded;
24 exact excerpts, layout, collection commands and original hashes remain.

Shared-host cache, frequency and memory effects were uncontrolled. These are
real XML fixtures processed by a fixed native driver or CPython consumer,
not full project executions. There is no confidence interval, hardware
mechanism proof, stable-production-toolchain claim or new full compatibility
certification.

## Reproduction and portable readback

`evidence.tar.gz` contains source, scripts, profiles, inputs, logs and audits.
`inventory.json` maps every logical file to its archive member, original path,
byte count and SHA-256. Absolute paths inside original reports are historical
provenance. They are not required by the portable verifier:

```sh
python3 -I -S verify.py .
```

The verifier loads no parser or compiler. It checks every archived payload,
both 70-file source maps, the pinned Expat source, every worker alias and raw
sample, and recomputes the 136 conditions' paired medians. `readback.json`
records its completed result. Original independent readers remain included
for their full build/vector/seed/order validations. Re-running an experiment
requires its recorded toolchain and library build prerequisites, adapting
historical absolute paths to a new isolated workspace; timing is not portable
or automatically rerun by this package.

Held-out XML fixtures retain all nine license/notice entries from their corpus
manifest. The six real training inputs retain their pinned project licenses,
authors and acquisition provenance; unused artwork bodies lacking an explicit
license are excluded by a separate inventory. Oriole Apache/MIT and upstream
notices, Expat's source notice and CPython's LICENSE accompany redistributed
source. No producer evidence is modified by packaging.

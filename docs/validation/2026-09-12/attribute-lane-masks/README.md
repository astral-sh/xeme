# Quoted-attribute SIMD and compact-owner experiments

## Scope and decisions

We select the quoted-attribute classifier at `910387239a804a038feb0a184b1bcfc0a7dd60bf`
after both real-project native and CPython epochs show modest aggregate gains with
unchanged compatibility outcomes. The generated entity regressions remain unresolved.
This is still experimental and does not meet the production-readiness or roughly
1.10× Expat performance goals.

The control is LF `cacc0b1b3766176e9cbf71d25c5ff6867fa87c08`, published unchanged by
`aed07bbacf75136330dd777a190ac2a42a2f1ed0`. Both the attribute candidate and the
separately rejected compact converted-name owner `5b503d74` are based on that
published commit; the prototypes are not combined. All use normal generic Oriole
O3/ThinLTO/one codegen unit with local Ohm rustc 1.98.1-dev and its experimental
defaults disabled. Expat 2.8.4 uses GCC 13.3 O3 without LTO. There is no new PGO work.

## Results

Ratios below one mean less time. Initial and confirmation measurements are separate;
no samples, process medians or condition ratios are pooled across epochs.

| Campaign | Real/Python time / LF | Time / Expat | Generated time / LF | Generated time / Expat | Adverse rows |
| --- | ---: | ---: | ---: | ---: | ---: |
| Compact-owner native, rejected | 1.012880× | 1.436608× | 1.033544× | 3.919038× | 18/28 |
| Attribute initial native | 0.976187× | 1.386529× | 1.038064× | 3.922022× | 7/28 |
| Attribute initial CPython | 0.984613× | 1.151785× | — | — | 5/24 |
| Attribute confirmation native | 0.976057× | 1.388594× | 1.030806× | 3.913642× | 5/28 |
| Attribute confirmation CPython | 0.984011× | 1.153534× | — | — | 4/24 |

Confirmation reduces real native time by 2.39% (21/24 wins) and CPython time by
1.60% (20/24 wins). Five of 24 CPython conditions meet the roughly 1.10× target;
both aggregates remain above it. ElementTree is 0.977396× LF / 1.204441× Expat
(10/12 wins), and pyexpat is 0.990670× LF / 1.104779× Expat (10/12 wins).
Generated native time increases 3.08%; entity fixtures regress 7.37% and 10.04%.
The nine confirmation regressions are:

| Consumer | Input | Chunk | Mode | Time / LF |
| --- | --- | ---: | --- | ---: |
| Native | batik | 4,096 B | namespaces off | 1.003881× |
| Native | generated-entities | 4,096 B | namespaces off | 1.073734× |
| Native | maven | 65,536 B | namespaces off | 1.006486× |
| Native | batik | 65,536 B | namespaces off | 1.005765× |
| Native | generated-entities | 65,536 B | namespaces off | 1.100403× |
| CPython | maven | 4,096 B | pyexpat-events | 1.005672× |
| CPython | maven | 65,536 B | pyexpat-events | 1.007823× |
| CPython | batik | 4,096 B | elementtree | 1.003355× |
| CPython | batik | 65,536 B | elementtree | 1.002537× |

The initial real native aggregate improves 2.38% (20/24 wins), and CPython improves
1.54% (19/24 wins). Generated native time regresses 3.81%; both entity fixtures
regress 8.22% and 9.07%. Initial ElementTree is 0.975437× LF / 1.200443× Expat;
pyexpat is 0.993876× LF / 1.105099× Expat. All 132 conditions and every one of the
39 adverse rows remain in the packet, including the rejected compact owner's 18.

## Measurement scope and host

These parse original XML from six pinned projects, not whole applications. Native
conditions use 4 KiB/64 KiB chunks with namespaces off/on; four generated controls
remain separate. CPython uses unmodified 3.12.13 O2 extension sources, ElementTree
and pyexpat at both chunk sizes. Each campaign uses seven seeded paired cohorts
and preserves its original warmups, iteration counts and measurement boundaries.
The six-row README table uses confirmation 4 KiB/namespaces-off medians of seven
process medians; paired-ratio medians are a distinct quantity. Aggregate ratios are
geometric means of condition-level median paired ratios. The five campaigns retain
3,168 workers and 371,256 samples, including preflight and warmup samples.

All elapsed work is pinned to CPU0 on the shared Linux AMD EPYC-Milan host. The
independent readers use only host-counter intervals wholly inside each elapsed
window. Before-run observations remain separate and chronological; percentages
below include I/O wait. During values are tick-weighted averages for CPUs other
than CPU0, not a measurement of exclusive host ownership.

| Campaign | Before: CPU0 idle + I/O wait | During: other CPUs idle + I/O wait | Elapsed window covered |
| --- | ---: | ---: | ---: |
| Compact-owner native | 95.38% / 97.49% | 96.80% | 95.35% |
| Attribute initial native | 94.87% / 95.23% | 93.18% | 91.44% |
| Attribute initial CPython | 93.18% | 95.94% | 96.57% |
| Attribute confirmation native | 96.38% | 95.69% | 96.57% |
| Attribute confirmation CPython | 97.59% | 98.68% | 97.26% |

CPU affinity, high idle-counter fractions and repetition do not establish isolation,
causality or statistical significance. Raw timestamps, both older pre-run observations
where present, boundary exclusions and per-interval ranges remain in the readers.

## Source and static evidence

The attribute change is only `tag.rs`: a 16-byte mask locates the first quote,
markup or uncertain byte; scalar handling preserves delimiter-before-invalid
ordering, both quote choices, short tails, incremental state and fallback timing.
The existing 8-byte tail and general/Unicode paths remain. Actual release
`TagScanner::scan` machine code grows from 3,373 to 3,638 bytes. This is a symbol-size
measurement, not a Rust object-layout measurement. The vector instructions are
static codegen evidence, not an executed-path or speed attribution.

The entity fixture has no attributes. In a separate bounded comparison, nine
Text/entity/C dispatch symbols retain exact instruction layout and non-address
operands; `TextPlan::scan` and `consume_text` are byte-identical at unchanged addresses.
Later symbols move by 0x110. This does not explain or dismiss the entity regression;
code-layout effects remain unproven. Complete machine-byte/PC-relative comparisons,
source pins and disassembly are retained in the entity-codegen record.

The compact owner replaces the immutable conversion owner using existing Shared
storage while preserving construction/drop order. Its actual debug Element size
falls from 120 to 88 bytes; Parser stays 2,464 bytes. Real native time regresses
1.29%, generated time 3.35%, so it was rejected. Smaller debug layout is not a
measured net-memory or speed win. Python source preparation exists but no compact
Python/full API/strict targets ran.

All three source identities contain 74 main files plus the same four auxiliary
manifest/lock files. Manifests and dependencies are unchanged from LF. The packet
retains exact checksum-pinned wide 1.7.0, safe_arch 1.2.0 and bytemuck 1.25.2 archives,
156 source files and licenses. Source/feature review does not establish execution
on every architecture or tested MSRV. No rejected compact-owner source enters the
attribute runtime. Existing source/build reviews bind the committed source and
actual libraries; the final packet reader verifies all three source maps.

## Compatibility and review

Attribute checks pass 456 Rust tests across 35 groups, formatting and strict
workspace/locked-benchmark Clippy. Independent saved raw review preserves all
4,740 original API rows: 4,347 pass, 391 assertion failures and two timeouts;
original raw exit 1, assertions and resource limits are preserved. Six shared/static
C consumers pass with C-only ASan/UBSan, uninstrumented Rust libraries and leak
checking disabled.

Strict shared/static CPython retains 802 method outcomes and 809 rendered lines
per linkage, including the same two callback-grouping failures/raw exit 2.
Two supplemental semantic tests pass separately per linkage. Strict modules carry
the explicit upstream allocation-failure cleanup backport; timed modules use
unmodified CPython sources. Supplemental passes do not turn strict failures green.

One agent authored the bounded classifier proposal; a separate agent reviewed the
source. Root composed the source and owned compiler, parser and elapsed execution.
Independent saved readers verify actual binaries, complete raw native/Python results,
host intervals and unchanged API/strict outcomes. The packet assembler author also
authored some source and saved-data studies; separate agents review the assembler,
final packet and documentation. These roles are recorded in their receipts.

Earlier sustained-fuzz, installed PBS, glibc and PGO/v3 results remain historical
source scopes. No current-source fuzz/PBS result is inferred. Source-preparation,
formatting and metadata/readback corrections are retained with their actual scope;
they are not parser failures. Current strict metadata was labeled correctly before
execution; the older LF label correction is history.

## Reproduction boundaries

The archive retains original worker stdout/stderr/specs, source snapshots, compiler
vectors, inputs, library identities, preflight and elapsed readers, host records,
compatibility logs and separate decisions. Excluded binaries/profiles and inherited
historical worker campaigns retain their identities. The existing deterministic
archive/index/original-byte readback implementation is reused. Assembly uses only
completed saved data and never executes a parser or compiler. Reproduction requires
the pinned inputs and separately retained excluded artifacts; it does not promise
byte-identical output from a fresh compiler build.

## Raw evidence

[evidence.tar.gz](evidence.tar.gz) contains 6,642 indexed files (16,946,975 compressed bytes). [index.json](index.json) records each member hash, original path, size and excluded artifact identity. Every member and original byte was read back. [conditions.csv](conditions.csv) retains all 132 conditions, and [adverse-conditions.csv](adverse-conditions.csv) retains all 39 adverse rows. Five campaigns remain separate; no observations or medians are pooled. [report.json](report.json) retains full precision.

Archive SHA-256: `ccae523af2136e76ceb614c759fb18671c506306640e37cea815596e425a1548`. Libraries, binaries, compiler profiles, unrelated historical worker records and machine configuration are excluded. Rebuilding the pinned sources does not promise byte-identical compiler output. Original readers use historical absolute paths; the member index maps archived inputs to their original names, and excluded artifact checks still require separately retained exact files.

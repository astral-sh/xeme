# Profiled custom-encoding composition

This checkpoint composes lexical provenance onto token ownership at `703a01a`.
Decoded raw callbacks publish only after lexical parsing completes. Plain tokens
exchange existing string owners; ASCII aliases decode fallibly before either
owner changes. Metadata remains available for all raw-name and lexical checks.

The shared library SHA-256 is
`11c17032b9f03eac3eb0a94d07355e11da737f9f08b15fc37563dc9bf507c0de`.
[The report](report.json), [evidence](evidence.tar.gz), [file manifest](files.json)
and [checksums](SHA256SUMS) retain the final source, intermediate profiles,
rejected experiment, complete API outcomes, converter probes and source review.
Earlier [original](../custom-encoding-provenance/) and
[lifecycle-composition](../custom-encoding-integrated/) evidence remains separate.

## Compatibility and safety

The exact token baseline completes all 4,740 upstream configurations at
**3,999 passing / 741 failing**. This candidate completes at **4,047 passing /
693 failing**, with exactly 48 failures fixed and no other outcome changes.
Adapted sources, assertions and allocation retry limits are unchanged.

The token baseline itself adds four failures relative to its preceding lifecycle
checkpoint: two parameter-entity reallocation tests at chunk size zero, with both
deferral settings. All four assert that parsing must fail when no reallocations
are allowed. Parsing now succeeds without needing Expat's crafted string-pool
reallocation. These are minimum-allocation assumptions, not upper retry ceilings;
a 512-ceiling diagnostic cannot change that first-iteration assertion. Original
source and failure logs remain retained. These four failures are not waived.

All **263 core/FFI tests**, strict Clippy and formatting pass. All **3,918 ordinary
baseline/candidate differential cases match exactly**. The 114,714 converter
configurations retain zero acceptance differences and the previously documented
5,112 successful external default-handler differences. Malformed-input callback,
error and position differences remain in their reports. Six native shared/static
ASan/UBSan suites pass with 333 allocation-failure scenarios per linkage. Rust
release code is uninstrumented and leak sanitizer is disabled.

Independent source review covers plain-path guards, sparse metadata, alias
normalization, typed defaults and fallible raw publication. It reports no finding
and claims no independent test execution.

## Profile-guided changes

Empty metadata now takes an inline plain-slice path; appending plain text skips
metadata reservation. Attribute normalization scanning follows an actual typed
attribute declaration lookup. Character data and raw-span copying keep their
original string views when no conversions exist, constructing lexical views only
for encoded data. Converted-input behavior and independent security budgets stay
active.

| Wayland instruction count | Instructions | Relative to token baseline |
| --- | ---: | ---: |
| Token baseline | 60,594,801 | 1.000 |
| Initial provenance composition | 66,396,916 | 1.096 |
| Plain slice and typed-default fast paths | 62,807,859 | 1.037 |
| Final text/raw-span fast paths | 62,098,050 | 1.025 |

The GTK count rises from 13,571,985 to 14,082,618 instructions, a remaining 3.76%
cost. A further attribute-helper experiment saves less than 0.1% total
instructions while adding branch complexity and changing alias-tag allocation
reuse; it was rejected. Its source and measurements remain in the archive.

Five randomized pairs on six pinned project inputs, using seven measured parses
per process on CPU 1, report median candidate/control throughput ratios of
**0.929–0.990 at 4 KiB** and **0.904–0.997 at 64 KiB**. Callback hashes, element
counts and text totals agree. These short shared-host screens retain substantial
individual outliers and show a remaining workload-dependent cost of roughly 1–10%.
They are native-driver comparisons with the previous Oriole runtime, not actual
project process timing or evidence of outperforming Expat. The complete combined
runtime still needs fresh project benchmarks.

The 693 remaining API failures and broader production-readiness work remain open.

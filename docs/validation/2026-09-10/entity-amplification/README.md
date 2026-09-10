# Consumed-byte entity amplification

This checkpoint implements Expat's maximum entity-amplification factor and
activation threshold. Root and external child parsers share direct and indirect
byte counts. Only consumed root input increases the denominator; trailing supplied
input cannot dilute entity expansion. Internal replacements, attributes, defaults,
parameter entities, external values and byte-order marks contribute at their
processing boundaries. Absolute input, expansion-work, allocation and nesting
limits remain independent.

The reviewed shared library is
`fcd58ad09d46263ae8b9df947a6e446bc3d3c27feedab8d323934e4566bfd223`,
built on `463a86a`. [The report](report.json) records source and library hashes,
commands and outcomes. [Evidence](evidence.tar.gz) includes the complete adapted
upstream test sources, observations, differential scripts, review, native logs and
earlier failing probes. Its per-file manifest and [SHA256SUMS](SHA256SUMS) identify
the retained artifacts.

## Compatibility

The unchanged Expat 2.8.4 public API suite completes all **4,740 configurations:
3,775 pass and 965 fail**. The two amplification-control tests now pass in all
12 chunk/deferral configurations, fixing 24 failures. All other outcomes and
adapted test-source hashes are unchanged from the preceding 3,751/989 checkpoint.
There are no crashes or timeouts. Allocation retry limits and assertions remain
unchanged.

Two differential grids cover 5,544 and 14,480 configurations, with no acceptance
differences. They exercise factor/threshold boundaries, direct and incremental
input, UTF-16, predefined and numeric references, nested entities, attributes,
defaults, external DTDs and external value storage. Twelve coarse-grid and four
fine-grid observations reject with amplification errors at different nested
parser levels; both libraries reject those inputs.

An additional external-value lifecycle difference predates this patch: repeated
external values containing an internal parameter reference leave that entity open
in Expat and trigger a later recursive-reference error. Oriole accepts the same
input with its existing truncated value behavior. The original probe and a
baseline/candidate/reference replay are retained separately; they are excluded
from the control-grid acceptance claim.

## Security and ownership checks

All **232 core/FFI tests**, formatting and strict affected-package Clippy pass.
Regressions cover trailing padding, raw UTF-16 widths, root-only setters, shared
sibling counters, children that outlive their parent, suspension/resumption,
callback changes, reset defaults, counter overflow and independent absolute caps.
The public C integration test passes against both pinned Expat and Oriole. It
tightens the relative limit from an element callback and verifies that unparsed
trailing padding cannot bypass it through either parse API.

All six shared/static native integration, adversarial and allocation runs pass
under C address/undefined sanitizers. Each linkage exercises 358 injected
allocation failures. The Rust release library is uninstrumented and leak
sanitizer is disabled.

Independent review found two edge cases during development: BOM-only children
initially escaped relative accounting, and a draft fix checked an explicit unknown
encoding before its recovery callback. Both are fixed. A separate 60-case probe
checks supported encodings and final/nonfinal feeds; its eight remaining timing
or diagnostic differences are unchanged from the initial candidate. No accounting
finding remains in the reviewed patch.

The 965 upstream failures, other compatibility boundaries and overall performance
remain separate work. This checkpoint does not establish broad drop-in readiness.

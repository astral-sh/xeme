# Shared parameter-family declaration state

Commit `d7686d2` gives parameter children immediate access to their family's
declaration-skip state. Precreated siblings observe a skip before the first
child finishes parsing. General-content children retain independent snapshots;
reset creates a new family while old children keep their owned state.

`ParameterState` combines the existing read marker and declaration-skip flag.
It initializes lazily before returning the first parameter child, using the
selected allocator. Allocation happens outside OnceLock initialization, and
both flags are charged to the existing work budget. Ordinary construction and
internal-only parsing retain the unallocated path. Sticky entity-value flags,
raw lexical decoding, boxed events, and default security limits are preserved.

## Validation

- All **283 Rust checks** and strict Clippy pass, including selected-allocator
  OOM, lazy initialization, work limits, siblings, snapshots, and reset lifetime.
- All four sibling cases now match Expat; the preceding library differed in
  three. Both general-child snapshot cases match, fixing one prior difference.
- An additional 27 creation-order/standalone conditions match Expat in both
  baseline and candidate. Deliberate skip-flag clearing during disabled
  processing follows the pinned Expat assignment.
- All 1,944 missing-PE oracle conditions retain the preceding results: no
  top-level acceptance differences, with 24 child-execution and 162 callback
  differences from external-subset timing and rejected-input prefixes.
- The complete API matrix remains exactly **4,065 passed / 675 failed**, with
  identical assertions, outcomes, and process dispositions.
- All 3,918 ordinary differential cases agree exactly with the preceding layer.
- Six native C suites pass, including 333 allocation-failure scenarios per
  linkage. C uses ASan/UBSan; release Rust is uninstrumented and leak sanitizer
  is disabled.

Independent source review found no blocker in lazy allocation, ownership,
declaration-state access, marker ordering, or retained lexical behavior. Its
exact file hashes and limited execution scope are preserved. No performance
claim is made for this state-sharing fix.

## Evidence

The frozen shared library SHA-256 is
`0407c70733d4d44fa9b590c45ccde13b9ae2e8dbb527f3087fea078c988420b4`.
`evidence.tar.gz` contains frozen sources, the original patch, integration
rejects and resolutions, complete API results, focused oracles, native logs,
and independent review. `files.json` lists every archive member and checksum.
Iterative attribute expansion and replacement semantics remain separate layers.

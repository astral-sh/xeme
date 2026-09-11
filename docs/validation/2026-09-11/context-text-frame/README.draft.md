# Context Text frame on corrected allocator: validation and performance

This draft is preparation only. Final assembly waits for both Python elapsed
phases and their independent audit. The assembled report will replace this
paragraph with the actual completion and selection decision.

The candidate resolves eligible ordinary UTF-8 Text directly from retained C
input context, while preserving frame reservation, eager raw publication and
accounting. The current source is commit0f on allocator repaire1, source72.
Its six intended changed paths include three runtime files, two unit modules
and a comment-only test-file change; the allocator bytes remain exacte1.

## Current completed evidence

- Combined CI: all16 jobs succeed at exact0f, including45 Miri tests per model
  (34 storage,4 core,7 C); warnings remain. The source06 packet separately
  retains all initial failed local tests, Clippy and both first Miri failures.
- Full API outcome parity:4,347 pass,391 assertion failures,2 resource-bound
  timeouts across4,740 rows, raw exit1. Both shared/static CPython802-method maps
  retain two known failures,3 expected failures,14 reported skips and raw exit2.
- Native normal real ratio0.996488 (9/24 adverse), generated0.985784 (0/4 adverse).
  Native PGO real0.986185 (5/24 adverse), generated1.005805 (3/4 adverse).
  PGO versus Expat1.370109. Ratios below1 are faster; allraw adverse cases stay.
- Native audit reproduces1,344 workers and212,352 samples, including exact
  callbacks, seeded order, warmups, medians and allpaired ratios.
- Fresh normal/original-G builds retain9 actual vectors,864 training rows,
  profiles, tools and source pins. The build reader shares build authorship;
  this limitation remains explicit.

## Python elapsed

Pending completed normal+PGO saved results and independent audit. No counts,
ratios or selection claim are filled from predictions or native measurements.

## Planned portable package

Use the allocator packet's verified deduplicated SHA-256-object archive.
Index binaries/caches byhash without copying them. Retain current source/build,
combined CI, full compatibility, native/Python raw results, independent readers
andall failures. Carry source06 historical packet unchanged with its own notices.
Reconstruct duplicated workers from byte-exact embedded records. A portable
verifier will check every object/source pin and replay saved statistics without
loading a parser. Exact final sizes, hashes and readback result follow assembly.

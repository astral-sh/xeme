# Inline immutable character data

Short character-data events now keep up to 23 UTF-8 bytes inside their owned payload.
Larger values retain the selected fallible allocator. General mutable strings remain
unchanged. The x86-64 sizes of String, attributes, events, and parsers do not grow.
This is an intentional early Rust API change from `String` to immutable `Text` in
`EventKind::Text`; [the migration note](API.md) describes read access and accumulation.

Seven alternating process pairs on CPU 3, with ten timed parses and a warmup per
process, compare the original, prior spare-NUL optimization, new Text variant, and
Expat. Full normalized callback preflight passes all 60 workload/chunk/implementation
combinations and native event digests agree. Reference-heavy C parsing is 1.16–1.18×
faster than the spare-NUL version across 64-byte, 4 KiB, and 1 MiB chunks. Element,
namespace, text, and uninterrupted-long-text results stay near the previous version
(0.98–1.05×). Reference-heavy allocation calls fall from 80,019 to 19. The safe-core
reference case also improves. Expat remains substantially faster overall; these are
paired optimization measurements on a shared host, not deployment latency guarantees.

The broader global inline-String experiment was rejected: it enlarged enclosing
objects and slowed safe-core reference parsing by roughly 8–15%, despite large
allocation reductions. Its initial and inline-hint variants, raw measurements,
source snapshots, sanitizer logs, and review remain in the negative-results archive.

The selected isolated Text variant passes 151 tests, 356 allocation-failure scenarios,
594 exact byte-boundary comparisons, and 71 ASan tests; all fuzz targets pass Clippy.
Independent ownership/provenance review and root source review find no blocker.
The combined header/declaration/Text source passes 186 workspace tests, formatting,
and strict Clippy. Shared and static CPython each run 803 XML tests with 31 skips
and three expected failures using the recorded upstream consumer cleanup backport.
The 12,318-case combined Expat differential has no status, error, normalized-event,
or callback-error discrepancies; six fragmentation and 450 location differences
remain. New sustained fuzz campaigns on the final combined source remain pending.

`review.json` identifies the measured isolated source and binaries. The integrated
source manifest and validation archive identify the later combined implementation,
which also contains the preceding DTD layers. Raw results, generators, patches,
source archives, and source/library hashes are retained alongside the summaries.

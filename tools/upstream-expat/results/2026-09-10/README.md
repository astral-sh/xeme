# Expat public API baseline, 2026-09-10

The pinned Expat 2.8.4 reference passed all **4,740 checks**. The frozen Oriole
`benchmark-reuse` library passed **3,095** and failed **1,645** assertions across
**145 distinct tests**. No tests crashed, timed out, or exceeded the resident-memory
limit after correcting the test allocator. This baseline predates the callback
guard and later compatibility fixes; it does not describe the final branch.

`summary.json` contains every distinct failing test and context, with categories
for follow-up review. Categories do not waive failures. Compressed results retain
all checks; compressed logs retain original assertion diagnostics. Manifests record
exact revisions, adapter and library hashes, compiler commands, process bounds,
and twelve named private-only exclusions. Both loaded-library origins were verified.

The original allocator-only ASan report is retained separately. The same standalone
reproducer exits successfully with the one-line tracking-list correction; the
corrected run log is empty and its manifest records exit status zero. No XML parser
is linked into either allocator-only probe. See the adapter README and memcheck.patch.

## After callback and API recovery fixes

The frozen `reviewed` library passed **3,311** and failed **1,429** checks across
**127 distinct tests**, again with no crashes, timeouts, or memory-limit kills.
This is 216 additional passing contexts. The compressed `reviewed-*` files retain
the complete updated matrix, hashes, and original diagnostics. Subsequent metadata
budget and lexical diagnostic changes are not represented by this checkpoint.

The [allocation audit](allocation-audit/) distinguishes low retry ceilings and
implementation-specific allocation schedules from behavioral defects through
separate, explicitly modified diagnostic probes. It does not waive original
upstream failures.

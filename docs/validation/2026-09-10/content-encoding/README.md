# Initialize external content with its encoding context

General external entities explicitly labelled ISO-8859-1 now decode leading
FF FE, FE FF and EF BB BF bytes as ordinary Latin1 data. Unlabelled external
content starting with an ASCII byte followed by NUL now fails as UTF-8 instead
of guessing UTF-16LE. BOMs, `<`-NUL markup signatures, leading-NUL big-endian
signatures and explicit protocols retain their existing behavior.

The decoder receives the existing parser context on each feed. This adds no
field, allocation or scan, and avoids losing the context when SetEncoding
replaces a decoder before input starts. Root, DTD and entity-value initialization
remain separate. Late declaration decoding and broader protocol/BOM conflicts
remain outside this patch.

All 186 core/FFI tests pass, including all-chunk safe-core and C ParseBuffer,
SetEncoding and parent-free regressions, selected-allocation failures and global
allocator bypass/reentry checks. Strict Clippy and formatting pass. The 61-case
oracle fixes 16 outcomes with no new differences; the 135-case context oracle
changes only general-content outcomes. All five targeted upstream tests now pass
in all 12 configurations. Four upstream controls continue to pass; three
intentional ASCII-converter/name-repertoire boundary tests retain their failures.

The patch contains five files and applies without fuzz to the immutable
final-values snapshot. Source hashes, frozen library hash, full observations,
commands and generators are included. Exploratory generators retain recorded
local source/library paths; the shared namespace/version probe they read is
included with the evidence.

The integrated release also passes the complete workspace and strict Clippy.
Both differential oracles reproduce all 196 isolated observations exactly after
the foreign-DTD, namespace, and XML version changes. The independent root review
records the resulting source/library hashes and checks the mode boundary and
bounds. This checkpoint is separate from the earlier benchmark and PBS runtime.

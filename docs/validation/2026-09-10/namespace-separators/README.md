# URI characters as namespace separators

We match Expat's legacy URI-character separators, including colon, while rejecting
collisions for non-URI separators. The check uses the expanded URI, so character
and entity references cannot evade it. Choose a non-URI separator such as `|` for
unambiguous expanded names; this compatibility rule is not full URI validation.

The root oracle covers all 128 ASCII separator values, default/prefixed/defaulted
namespace declarations, attributes, three chunk sizes, and triplets. Its 3,060
observations match Expat exactly, fixing 1,590 baseline differences with no
regression. NUL with triplets remains an explicitly unsupported constructor mode
and is excluded from this matrix, not counted as a pass.

Independent review compares the character set with pinned Expat 2.8.4 and runs
189 further cases with Unicode URIs, general entities, character references,
UTF-8/UTF-16 and varied chunking. Every observation matches. All 12 upstream
namespace-separator contexts pass, as do focused Rust regressions and strict
Clippy. Raw inputs, callbacks, baseline observations and source/library identities
are preserved in the compressed evidence.

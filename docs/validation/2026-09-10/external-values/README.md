# External parameter entities in values

External references inside entity values now create separate value children and
resume the owned declaration after the callback returns. An allocation-free
`TryLock` protects their shared output; source input never crosses a callback as a
borrow. Child initialization, skipped input, recursion, expansion budgets, custom
encodings, suspension, and allocator failure remain separate state transitions.

This layer builds on the storage primitive in PR #43 and the declaration Default
callbacks in PR #42. The frozen combined source and release-library hashes are in
`source.json` and `summary.json`. All 208 workspace tests and strict Clippy pass.
The source archive excludes binaries; release build commands and logs are retained.

## Independent validation

The isolated implementation passes 18,468 UTF-8/UTF-16 cases with no status, error,
or normalized-event differences. Its focused upstream matrix improves from
108/144 to 132/144 passing configurations. The remaining test fails later in
ordinary external-DTD diagnostics; all 22 extracted value-child cases match.
The lexer has a 73,360-case tokenizer comparison and a separate 50,000-case random
extension. The former retains existing differences in XML name repertoires.

The combined review runs 3,456 grammar/value cases, 6,912 Default-handler cases,
and the original 2,880-case declaration Default matrix. All successful events and
parse outcomes match. Respectively 768, 1,152, and 48 malformed-input callback
differences reproduce the earlier Default baseline. These matrices are separate,
overlapping contracts, not additive coverage counts.

Integration review found and fixed duplicate prefixes, lost trailing grammar
references, a missing-Default-handler panic, and an unread child's following
whitespace being hidden by an entity handler. Root regressions cover these cases,
including handlers installed or removed during a child callback. `combined-review`
retains the failing and corrected observations, generators, and source audit.

## Retained boundaries

The isolated primary matrix matches 787/788 family status sequences and all 481
completed successful event sequences. One fragmented X-NUL autodetection case,
279 partial parent callbacks after external-handler failure, and 12 unfinished
XML-declaration timing cases remain. Position and root NotStandalone timing are
outside that normalized comparison.

The supplemental 54-case encoding matrix retains 24 acceptance differences for
encoding declarations following already-decoded value content. Six leading custom
encoding cases differ in encoding-handler versus XML-declaration callback order;
nine malformed UTF-8 cases share the partial-parent callback limitation. Generic
external parameter references between declaration grammar tokens remain unsupported.
The complete isolated handoff describes each boundary and retains exact evidence.

These results do not establish complete Expat compatibility. Combined consumer,
full upstream, sanitizer, and distribution runs are recorded in subsequent layers.

# Prolog literals and reference deferral

Quoted tokens outside a declaration now retain Expat's lexical error precedence,
including unfinished literals, invalid closing delimiters, encoding failures,
and byte positions. Incremental scanning retains its checked prefix and enforces
the token budget. Entity references now participate in geometric reparse deferral,
matching the callback delivery before later malformed text.

The 60,159-case expanded corpus has no acceptance or normalized callback
differences. It retains 39 error-code differences caused by Expat's older XML name
repertoire, three exact callback-fragmentation differences, and 2,307 final-location
differences. Compared with the preceding diagnostic layer, 279 cases lose all
observed discrepancies and no previously matching case gains a discrepancy.
Two already-mismatching cases change their observations. Neither the semantic
nor strict compatibility gate is marked passing while these differences remain.

The focused oracle covers 4,248 quoted-prolog cases and 672 reference/text patterns
without an observed difference in the checked fields. All 137 core and C-interface
tests and strict Clippy pass. UTF-16, all input splits, lexical error precedence,
small limits, and one-byte long literals have regression coverage.

Compressed evidence includes exact minimized inputs, probes, all expanded
mismatches, comparison code, logs, source hashes, and the patch against the preceding
diagnostic snapshot. The expanded corpus is retained by that preceding layer;
large complete worker outputs remain preserved locally with hashes.

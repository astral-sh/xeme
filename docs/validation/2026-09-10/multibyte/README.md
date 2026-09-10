# Custom multibyte encoding validation

The safe parser exposes an owned two- to four-byte conversion request. The C wrapper
calls the application's converter after releasing the core borrow, then resumes
parsing. Input compaction and cached decoded-end positions keep conversion linear;
original byte widths preserve source locations. Tests cover every chunk boundary,
non-ASCII names/attributes/text, partial and invalid sequences, callback reentry,
release ownership, parent/child lifetimes, and allocation failure.

Independent review found that converting ASCII aliases before tokenization can
change XML syntax: keywords, XML declarations, references, and namespace names depend
on the original byte form. The implementation rejects every multibyte-to-ASCII
conversion before tokenization. Direct ASCII map entries and valid non-ASCII BMP
conversions are supported. This is an explicit compatibility boundary, not complete
Expat custom-encoding support. The reference-only lexical probes retain examples.

The focused upstream matrix has 216 passes and 36 failures across 252 checks; the
same reference matrix passes all 252. The three failing tests require ASCII aliases:
`test_unknown_encoding_success`, `test_unknown_encoding_long_name_1`, and
`test_misc_unknown_encoding_callbacks_protected`. All twelve contexts are retained
for each failure. These focused results use the recorded debug-library hash, before
the final conditional-section review corrections.

The complete matrix against the final combined release library has 3,455 passes
and 1,285 failures across 115 distinct tests, with no crashes or timeouts. Its manifest
and full results are included. The original 4,740-check reference run and harness
adaptations remain in the preceding validation checkpoint. Shared/static CPython
consumer results and dedicated sanitizer campaigns are recorded separately.

`source.tar.gz` and the compressed source/build manifests identify the final library;
`index.json` hashes every report. The source archive includes the separately staged
fuzz target, which does not contribute to the library build. Earlier focused matrices
and reference-only probes keep their own hashes; they are not presented as runs
against the final release binary.

# Reference and end-tag diagnostics

Malformed reference and end-tag prefixes now report invalid-token errors at the
first offending character. Valid unfinished prefixes remain unclosed-token errors;
a later decoding failure does not override an earlier lexical error. Scanning
resumes at UTF-8 boundaries, without rescanning previously checked characters.

The 12,159-case generated replay passes status, error code, and normalized callbacks.
It retains 58 exact callback-fragmentation and 480 location differences. The focused
regressions, full core/C-interface tests, allocator failure sweeps, formatting, and
Clippy pass. The source manifest and patch identify the isolated implementation
relative to the preceding multibyte source archive.

Independent review covers 17,494 UTF-8/UTF-16, Unicode, chunk-size, and deferral cases.
It finds no new reference discrepancies and fixes 5,210 previously mismatching
cases. Remaining discrepancies include Expat's older XML name character repertoire
and error positions after incomplete decoding; the complete observations are retained.

A subsequent 60,159-case generated run has no acceptance differences, but retains
54 error-code and two normalized-callback differences. Thirty-nine error differences
involve Expat's older name rules after UTF-16 autodetection, and fifteen involve
quoted text in the prolog. The callback differences concern text delivered before
malformed input under reparse deferral. Follow-up work addresses those separate
prolog and scheduling behaviors; they are not waived by this layer.

Compressed reports retain exact inputs, all mismatches, probe sources, library
hashes, and review comparisons. `index.json` hashes the reports and records the
local paths/hashes of the complete expanded worker outputs. The expanded corpus
and its full mismatch summary are committed; the larger worker outputs remain
preserved locally. These runs do not establish complete Expat equivalence.

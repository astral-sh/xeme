# External references in conditional headers

External parameter references in conditional headers now load a separate DTD,
import its declarations, and resume the retained parent header. Child bytes do
not become conditional-keyword text. Owned continuations preserve callback
suspension, input buffers, child lifetime, active entity ancestry, and shared
allocation and expansion limits.

The isolated implementation passes 149 core/C-interface tests and strict Clippy.
The integrated header layer passes its seven focused core tests and formatting.
All 192 targeted upstream Expat configurations pass, up from 168 at the baseline.
The 1,836-case primary oracle fixes 1,156 status/error discrepancies without a new
one. Parent status/error and scoped callback results all match; 44 child diagnostic
differences remain (Syntax versus InvalidToken). The separate prolog change does
not fix those external-DTD diagnostics.

All 576 mixed-header status/error results match; 12 retain duplicate-declaration
Default fragment differences. Independent review covers another 54 nested-read
and absent-handler cases, mixed headers, inherited recursion, ownership, and
resource bounds, with no blocking finding. Root NotStandalone ordering and
malformed callback timing remain outside this layer's compatibility claim.

The archive retains source snapshots, raw observations, generators, upstream logs,
and the isolated patch. `manifest.json` identifies the measured isolated library;
`integrated-source.json` identifies the source in this PR. They are distinct:
the integration also contains the preceding prolog and callback-allocation layers.

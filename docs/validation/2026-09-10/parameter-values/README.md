# Internal parameter entities in entity values

Internal parameter replacements now expand inside entity values in legal external
DTD and parameter-entity contexts. Borrowed frames preserve lexical boundaries and
normalize physical line endings once. Replacements use shared expansion/work/depth
limits and the selected fallible allocator. Missing references preserve the current
declaration and its suffix while suppressing subsequent declarations when required.
Standalone declarations preserve Expat's parameter-mode inheritance behavior.

Independent review observes 256/256 matching status/error cases, 12/12 exact child
mode-inheritance cases, and 23,328/23,328 normalized encoding/chunk/mode summaries.
These results do not waive the retained partial-callback differences on malformed
or duplicate declarations. A separate resource review finds no blocking issue;
exponential, empty, and missing replacement workloads hit the shared work limit.

The focused upstream matrix improves from 40/180 to 64/180 passing configurations,
with 24 newly passing configurations and no regressions. The reference passes all
180. Remaining failures, including external-value continuations and exact allocator
threshold assumptions, are retained in the implementation report.

The isolated implementation passes 141 core and C-interface tests, allocator failure
sweeps, formatting, and strict Clippy. The combined prolog/parameter-value source
passes those test suites and Clippy again. Evidence includes exact input generators,
observations, reviewed hashes, the isolated patch, and the combined source manifest.
Arbitrary declaration fragments and external references in values or conditional
headers remain separate compatibility boundaries at this layer.

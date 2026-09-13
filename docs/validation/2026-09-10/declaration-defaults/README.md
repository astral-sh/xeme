# Declaration Default callback fragments

Each raw declaration byte is now assigned once across skipped parameter references.
Handler-dependent Entity/Attlist prefix events preserve callback order, and duplicate
entity declarations retain Expat's sparse name/literal/closing tokens when an EntityDecl
handler is installed. Complete raw text is retained when it is absent. Storage remains
fallible and allocator-owned; queue traversal advances monotonically and added event
metadata is charged against the expansion budget.

The six original Default-only cases and 12 repeated-header duplicate cases are fixed.
All 12,784 UTF-8/UTF-16 chunk cases and 576 header replays match. The broader 2,880-case
handler/mode/standalone matrix has no acceptance/error or successful-event differences;
48 malformed-prefix callback timing differences remain. Another 96 NDATA-handler cases
confirm duplicate projection; 24 retain a baseline first-unparsed-declaration Default
omission, independently reproduced against the earlier diagnostic source. Neither
existing callback limitation is waived by these successful-input results.

The isolated source passes 165 core/C-interface tests, strict Clippy, and formatting,
including allocation-failure and allocator-reentry checks. Root source review finds no
blocking ownership, bounds, resource, or pending-event traversal issue. The integrated
Text/DTD source passes 166 core/C-interface tests, strict Clippy, and formatting.
Exact positions, malformed-prefix timing, and arbitrary mid-token handler mutation
remain separate compatibility boundaries. External-value continuation is a later layer.

The archive contains source snapshots, the isolated patch, full oracle observations
and generators, and validation logs. Separate manifests identify the measured isolated
library and the combined implementation committed in this PR.

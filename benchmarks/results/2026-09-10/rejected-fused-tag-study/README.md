# Rejected tag-validation fusion

The candidate combines completed-tag XML-character validation and line/column
calculation. It reduces instructions by 2.73% across twelve project/namespace
conditions, but takes slightly longer in the paired native timing screen:
geometric speedup is **0.98985×**, with 6 of 24 project conditions improving.
The two generated declaration conditions regress to 0.94725× and 0.94186×.
The runtime change was rejected. Lower instruction counts did not establish a
throughput improvement, and the cause of that disagreement was not determined.

The study compares baseline `9cc87e9b` with candidate `a641e2e7`. It retains all
280 timed processes, 56 semantic preflights, fixed per-process iteration counts,
unfiltered samples, source/driver/library hashes and callback hashes. Correctness
checks include 340 affected-package Rust tests, 1,518 strict differential cases,
36,456 custom-encoding cases, 95,548 per-feed tag conditions, the unchanged full
4,740-row API matrix, and six native C sanitizer/allocation processes.

The archive also preserves the original read-only structural profile and proposal.
That proposal preceded this failed experiment; its opportunity estimates are
hypotheses. In particular, the broad position/accounting category contains required
work, and Expat also updates positions at feed boundaries. It is not evidence that
removing position calculations can eliminate that entire category.

The [subsequent design assessment](structural-design/DESIGN.md) reclassifies the
same frozen profiles, gives the corrected position comparison, and proposes a
shared tag plan with detached owned callback storage. It contains no prototype
or new performance measurement.

`files.json` records both original manifests and every member in `evidence.tar.gz`.
Compiled binaries are omitted; their identities, source, commands, raw profiler
output, unfavorable results and original review limitations remain available.

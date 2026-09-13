# Independent prolog suspension review

This review compares the prolog/reference-deferral source with its diagnostic
baseline and Expat 2.8.4. All source and library hashes, the exact probe, and all
observations are retained. It supplements the larger generated replay; the two
campaigns exercise different contracts and their results are not interchangeable.

Among 564 callback stop/resume cases, 80 prior discrepancies are fixed and 242
remain unchanged. Two new discrepancies affect malformed input with a text callback
that suspends on every event: for `<r>&#x41;<n a="x">e]]>`, chunk widths one and four
with deferral enabled deliver an extra `e` callback before failure. Both parsers
reject with invalid-token error 4 at byte 21. These differences are retained;
exact malformed-input callback equivalence is not claimed.

The source review finds bounded incremental scanning, preserved UTF-8 boundaries,
and no blocking resource-limit or valid-input issue in the reviewed scope.
Remaining differences also include abort positions and CR/LF whitespace callback
granularity. These are compatibility limits, not waived passing cases.

# CPython results for rejected serialized accounting

The parent [experiment report](../README.md) explains rejection and includes both build modes. [conditions.csv](conditions.csv) retains all 48 condition ratios; [report.json](report.json) groups results by build and consumer. All regressions are retained.

The archive includes raw compressed worker output, extension compiler commands, source snapshots, original reviews and both strict-linkage logs. There are 1,152 preflight/timing workers and 52,728 samples across normal and PGO. Strict suites each report 802 tests, two failures and 14 skips and exit 2. All 809 rendered outcomes match the measured suffix control, including three expected failures; matching outcomes do not make the strict gate green.

Only strict consumers include the explicit cleanup backport. Performance extensions use unmodified CPython sources. Canonical checking coalesces adjacent text callbacks. Creation/feed/finalization/callbacks and destruction are timed; reads/imports/validation/explicit GC are outside, with automatic GC enabled. These measurements do not represent whole applications.

Using Python 3.12 from this directory:

```console
python3.12 -I -S verify.py --package . --output recomputed.json
```

The copied packet works without the original filesystem. The verifier checks every archived file, source snapshots, saved compiler vectors and original worker arithmetic, then compares strict outcomes to suffix. The replay reads JSON and compressed text only. It imports no parser library and reruns no benchmark. Compiled binaries are excluded; actual module/library byte checks belong to the archived original independent reviews. Strict origin logs record initial/fresh module hashes, while the performance workers additionally record XML_Parse library origins; strict logs do not provide that same-process dladdr check.

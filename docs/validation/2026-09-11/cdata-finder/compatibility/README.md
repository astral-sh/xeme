# Cached Finder compatibility evidence

The candidate retains the selected streaming baseline’s complete compatibility outcomes. All target commands ran once and were reaped.

- **C API:** 4,740 original rows: 4,347 pass, 391 assertion failures and two timeouts, byte-identical to the baseline. Original 3-second case, 1 GiB address-space, 768 MiB RSS and 240-second total limits remain.
- **C consumers:** six shared/static runs, including PR125’s nested-entity success regression and 327 selected-allocation scenarios per linkage.
- **Traces:** 3,318 strict cases, 36,456 custom-alias comparisons, 2,392 malformed pairs, 1,304 publication cases/3,912 parses, 38 End lifecycle cases/114 parses, and six End allocation scenarios match the baseline.
- **Strict CPython:** shared/static each retain all 802 method outcomes, including `BufferTextTest.test1` and `CDATAHandlerTest.test_handlers` failures, three expected failures and 14 reported skips. Both use the same pinned allocation-failure cleanup backport. Raw exit 2 is retained.

C wrappers use ASan/UBSan; the Rust PGO library is **not instrumented**, and leak checking remains disabled. Successful custom-alias raw pairs are not emitted by the inherited helper. Reference callback-position differences remain recorded. This is outcome parity, not an all-green test suite or a whole-Rust sanitizer claim.

The [report](report.json) identifies the collector’s detailed trace review and root’s separate independent API/strict review. The [archive](evidence.tar.gz) contains 388 source, controller, raw-output, method-map and review members. [Member and origin hashes](members.json.gz) also record 25 excluded compiled/cache files. No deduplication or alias reconstruction is needed.

Run `python3 verify.py` after copying this directory’s compact files. `python3 verify.py --check-origins` additionally verifies the original local paths when available. Neither command executes archived targets. The full gate controllers and their original absolute-path bindings are retained as evidence; a new execution requires a separately prepared environment and adaptation of the recorded paths.

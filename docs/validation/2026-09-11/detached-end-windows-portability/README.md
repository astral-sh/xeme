# Detached-end Windows test portability supplement

PR115's [Windows job](https://github.com/astral-sh/oriole/actions/runs/34553541588/job/103121238410) failed while compiling the callback-budget test: `XML_GetCurrentByteIndex` returns `c_long`, which is `i32` on Windows, but the expected offset was cast to `i64`. We cast the expected value to the already imported `c_long`. The fixture, expected offset, budget and assertions are unchanged.

This is a supplement to the [detached-end report](../detached-end-frames/). The prior source manifest `34473026` and all historical archives remain unchanged. The new 70-file manifest is `eab8c027`; only the test file differs. No runtime implementation changes.

The focused End callback-budget test passes on Linux, and formatting passes. A fresh C-only ThinLTO build emitted all three workspace compiler invocations with matching flags and produced byte-identical shared (`02fcab59`) and static (`69ebed41`) libraries. This preserves the measured C-library identity without repeating the full Linux gates. Windows and other platform checks run in [PR115 CI](https://github.com/astral-sh/oriole/pull/115/checks).

[The structured report](report.json), [one-line patch](test-only.patch), [source manifest](source.json), and [evidence archive](evidence.tar.gz) retain the original Windows failure, commands, raw local logs and source/build lineage. The archive contains no executable libraries; both binary hashes and full member readback are recorded. Local validation and packaging were performed by the patch author.

An [independent source review](source-review.json) verifies the exact one-line change and all 70 source/archive hashes. It is separate from the author-run build and focused test.

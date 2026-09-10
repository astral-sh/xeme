# Event output composition and attribution

This candidate combines caller-owned event output with three smaller changes: skipping a redundant zero-byte accounting call, avoiding an empty parameter-record copy, and searching attribute literals for two fixed marker bytes.

The combined source passes 326 Rust checks, strict Clippy, full custom-encoding and boundary probes, six native C sanitizer runs, and preserves all 4,740 upstream observations at 4,113 passing and 627 failing. Its shared CPython suite retains only the two established text-fragmentation failures; the existing semantic gate passes. Frozen source, commands, original logs and complete callback preflights are included. The custom-alias campaign was also rerun in isolated managed Python, with an explicit check against preloaded Expat; both attempts are retained and the final 36,456 comparisons are unchanged.

The combined native project screen improves all 24 conditions, with a 1.053× geometric speedup against the preceding parser and 2.49× Expat's time. However, a separate same-base comparison against event output alone finds only a 1.003× geometric gain from the three additional changes, with 13 of 24 conditions improving. The generated declaration fixture slows by about 5% in both chunk conditions. We therefore exclude those three changes and retain event output alone for the next final integration.

Both screens use five randomized process pairs and seven measured parses after warmup, with CPU 0 on a shared Linux host. All raw samples, source/library hashes, callback observations and generated controls are retained. CPU frequency and host load are uncontrolled. These results do not establish that Oriole is faster than Expat, and the combined candidate is not the selected runtime.

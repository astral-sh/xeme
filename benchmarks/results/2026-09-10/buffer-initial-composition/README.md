# Initial buffer composition

This is the first composition of lazy UTF-8 input/context reservations with streaming ATTLIST publication. Its shared library is `8d2ab7be6545e44f47fc3d7866ac32e3977d018629d6357333b873f3d323ed5b`; the baseline is the `0546fcf5` ATTLIST library. This source is superseded by the final captured-type reservation repair.

The full original upstream matrix passes 4,303 configurations and fails 437. Compared with the ATTLIST baseline, 200 configurations improve and 12 namespace binding reallocation assertions regress. Compared with the independently tested buffer-only candidate, this composition loses 20 expected enumeration/default allocation improvements. Allocation tracing identifies repeated growth of the captured enumeration type; the subsequent fix reserves a bounded suffix without retaining whitespace or later defaults. Original assertions are unchanged.

All 367 workspace Rust checks, strict Clippy and formatting pass. The archive retains original API records, native C sanitizer checks, exact and custom-encoding comparisons, streaming suspension/DefaultCurrent probes and active declaration/publication comparisons. Reports retain their tested binary identities and known reference differences.

Five randomized native cohorts cover six original project files at two chunk widths and both namespace modes, plus two generated controls. The geometric project speedup is 1.0055×, with 16 of 24 conditions positive; Oriole takes 2.5054× Expat's time overall. This near-neutral result does not establish a throughput improvement. All normalized preflights finished before timing. A short-lived compiler parent may have run on CPU 0 during collection; `collection-notes.json` retains that limitation. The host is shared, with uncontrolled frequency and external load.

These timings belong to this intermediate binary, not the final capture repair. Sources, commands, original logs, callback hashes and every raw timing sample are retained. No new full CPython, W3C, sustained fuzzing or distribution campaign belongs to this intermediate report.

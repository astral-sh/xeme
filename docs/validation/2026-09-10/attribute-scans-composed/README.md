# Attribute batching and fixed-delimiter search

Ordinary UTF-8 attribute literals copy contiguous runs between XML whitespace markers. Converted ASCII aliases retain their provenance-aware scalar path. Character data searches for the fixed `]]>` delimiter with the existing pinned `memchr` dependency, preserving its byte offsets and error precedence.

The final integration passes 323 core/adapter Rust checks, strict Clippy and formatting. All 4,740 upstream API observations are unchanged: 4,113 pass and 627 fail. Existing ordinary, attribute, malformed, custom-encoding, source-accounting, DOCTYPE and child-publication probes have no changes against the preceding runtime. Six native C ASan/UBSan runs pass, including 336 selected-allocation scenarios per linkage. Rust is not instrumented in those C runs and leak sanitizer is disabled. Existing reference position and callback differences remain explicit.

Across six pinned project XML files, two chunk sizes and both namespace modes, the combined native screen improves its baseline by 8.1% geometrically, with 13 of 24 conditions improving. Batik improves by 1.54–1.63×. Other project conditions range from small regressions to small gains. Oriole still takes 2.62× Expat's time on average. Generated entity/declaration controls also retain their measured regressions; this is not a claim that every input is faster.

A separate current-base attribution compares the fixed-marker search with attribute batching alone. It improves 23 of 24 project conditions, with a 1.038× geometric speedup; the generated declaration fixture is about 4% slower in both chunk conditions. The original isolated attribute study measured an 11% geometric project gain and 43% fewer Batik XML_Parse instructions. Those older-base results remain separately identified and are not multiplied into the current result.

Namespace name reuse was also composed and independently tested, but rejected after a current-base comparison showed a 0.9915× geometric project speedup. Its source, full compatibility checks, allocation reductions, original positive screens and later negative attribution are retained under the namespace composition evidence.

All project timings use five randomized process pairs, seven measured parses per process after warmup, CPU 0 on a shared Linux host, and full normalized callback preflights. Host load and CPU frequency are uncontrolled. Frozen source, exact library hashes, raw samples, commands and independent source reviews identify every build. The CI runner layer inserted during this work only changes commit identities; `restack.json` verifies runtime source bytes.

No new full W3C, sustained Rust sanitizer fuzzing, CPython distribution, or PBS campaign is claimed for this layer. Earlier broad validation remains attached to its original runtime.

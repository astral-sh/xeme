# XML name edition compatibility

Source commit `983f699` composes the original NameRules implementation onto the
final custom-encoding provenance layer (`261a7c1`). The C interface selects XML
1.0 Fourth Edition name rules to match Expat; the Rust interface retains its
Fifth Edition default and exposes `Config::name_rules` for either choice.

The selected rules reach raw names, namespace-qualified names, references, DTD
grammar, incremental scanners, and external children. Custom-encoding names
retain their original lexical spelling before decoded semantic lookup. Name
validation removed from decoded namespace expansion remains removed. Limits,
allocation primitives, and entity-expansion policy are unchanged.

Fourth Edition ranges are generated from W3C productions 84–89, independently
of Expat's implementation. Original production data, the union generator,
integration rejects, and their resolutions are retained in the archive.

## Validation

- All **273 Rust checks**, strict Clippy, and the streaming fuzz target compile
  check pass. The target now selects the edition independently of namespace
  mode. This checkpoint does not claim a runtime fuzz campaign.
- The full upstream matrix improves from **4,047 passed / 693 failed** to
  **4,065 passed / 675 failed**, with exactly 18 improvements and no other
  outcome or process-disposition changes. Adapted sources and assertions are
  identical. Six deferral-enabled configurations of `test_utf8_in_start_tags`
  skip their original test body; these existing passes are not new evidence.
- A fresh scalar oracle parses all **1,111,936 non-ASCII Unicode scalars** at
  both initial and continuation name positions: **2,223,872 cases per library**.
  Candidate and pinned Expat acceptance bitmaps match exactly. Every rejected
  input reports error 4, and rejected-error counts match. Surrogates are skipped.
- The **6,864-condition context oracle** covers DTDs, references, namespaces,
  internal and external content, UTF-8/UTF-16, and whole/one/seven-byte feeds.
  It retains exactly the original 180 missing-parameter-entity differences.
  That separate semantic fix is not included in this layer.
- All **3,918 ordinary differential cases** agree exactly with the preceding
  custom-provenance library, including status, errors, callbacks, and position.
- Six native C suites pass under shared and static linkage, with **333
  allocation-failure scenarios per linkage**. C uses ASan/UBSan; release Rust
  is uninstrumented and leak sanitizer is disabled.

## Custom PUBLIC identifiers

The focused regression maps `@` to a two-byte sequence and `^` to U+200C, while
the converter maps `@^` to `A`. Expat's PUBLIC scanner classifies the original
bytes: `@` is permitted punctuation, but the mapped character for `^` is a name
character only under Fifth Edition. Fourth Edition therefore rejects the
PUBLIC identifier with error 32 even though its decoded text would be `A`.

The Rust regression checks both editions. A separate **72-condition C oracle**
covers three mapped-character classes, four declaration contexts including
external children, both namespace modes, and three feed widths. Candidate
acceptance and error codes match pinned Expat throughout, fixing 24 differences
from the preceding library. Conversion and release counts are retained without
claiming exact converter-callback timing.

## Performance screen

Five randomized pairs use seven measured native-driver parses plus a warmup per
process, pinned to CPU 1. Callback hashes, element counts, and text-byte counts
agree. Ratios show candidate throughput divided by the preceding Oriole
custom-provenance library's throughput.

| Project input | 4 KiB feeds | 64 KiB feeds |
| --- | ---: | ---: |
| Vulkan | 0.998 | 0.997 |
| Wayland | 1.012 | 1.007 |
| Maven | 0.994 | 0.981 |
| Batik | 1.004 | 0.978 |
| GTK | 0.994 | 0.983 |
| DocBook | 0.982 | 0.966 |

All samples, shared-host outliers, and adverse generated fixtures are retained.
These are native callback-driver measurements on pinned project XML, not project
process timings or an Expat performance comparison.

## Evidence and review

The frozen shared library SHA-256 is
`2486ac0b7775389466996a2259af351011560edddbad44677a934ab2b10a893e`;
the static archive is
`18317c1ebdc838afece1c7873fdc5210fc8a826be25809e2108d3f31919e3d2e`.
`evidence.tar.gz` contains frozen sources, commands, exact API results, scalar
bitmaps, context and PUBLIC traces, allocation logs, benchmarks, and review.
`files.json` lists archive members and checksums.

Independent review verified the table unions and lookup, ASCII fast paths,
lexical representatives, and PUBLIC byte classification. It found no blocker;
its limited scope and final-format addendum are retained. A separate source
audit enumerates mode propagation across the parser, source frames, DTD/value
scanners, and child construction. Neither audit claims independent execution of
the author's Rust or differential suites.

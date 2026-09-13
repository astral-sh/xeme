# Current streaming-policy ASan follow-up

The current source passed a fresh Rust AddressSanitizer build, 13 selected policy regressions, retained-corpus replay on seven targets, and seven new 120-second fuzz campaigns. There were no failure artifacts or target retries. This is a **collector-owned execution report**, not an independent campaign audit.

The canonical 70-file source manifest is `e52c4fa16e71a7dcd76864b29509b157a42b6fbe4531f87b68f98395418d5fc3` and remained unchanged. Actual compiler records cover the three workspace crates and all seven fuzz binaries. Symbol checks cover all seven binaries; representative parser and adapter disassembly is retained. No historical sanitizer build is treated as current-source evidence.

## Outcomes

| Target | Replay executions | Campaign executed units | Final disk corpus files |
| --- | ---: | ---: | ---: |
| `ffi` | 8,682 | 369,549 | 568 |
| `ffi_family` | 10,589 | 96,844 | 174 |
| `multibyte` | 12,813 | 228,614 | 277 |
| `parse` | 8,793 | 723,605 | 775 |
| `streaming` | 11,782 | 295,674 | 440 |
| `streaming_work` | 11,782 | 332,705 | 513 |
| `value_family` | 4,456 | 123,740 | 184 |
| **Total** | **68,897** | **2,170,731** | **2,931** |

The 68,893 retained seed files are counted per target. `streaming_work` deliberately reuses the streaming seed set. Replay counts include libFuzzer initialization; campaign executed-unit counts separately include campaign initialization and mutations. All raw logs and exact commands are retained.

The six original harnesses are byte-for-byte unchanged. The seventh is a copy of `streaming` with only `max_entity_expansion_bytes` changed from 65,536 to 64 and `max_work_amplification: Some(100)` added. Its whole-input versus incremental oracle is unchanged. The 13 exact regression tests cover consumed-root credit, overflow, per-source/request bounds, converted positions, family reset and old-child separation, and namespace/default work.

## Bounds and limits

Each new campaign used `-max_total_time=120`, seed 20260911, maximum input 65,536 bytes, per-input timeout 10 seconds, RSS limit 1,536 MiB, and `ASAN_OPTIONS=detect_leaks=0:abort_on_error=1`. Retained outer bounds were 180 seconds per replay, 690 seconds per campaign, and 1,920 seconds for the aggregate. The regression commands had 1,200-second outer bounds. The controller owned process groups and reaped all workers; no deadline was reached.

These seven new 120-second campaigns are a scoped incremental follow-up, **not equivalent to the historical six 600-second campaigns**. Historical seed metadata and bytes are retained only for seed provenance. Leak detection was disabled; this does not claim LSan, UBSan, whole-standard-library instrumentation, PGO validation, performance, or exhaustive safety.

The preserved raw instrumentation report has an inherited sentence saying “six fuzz targets”; its seven-binary map and actual compiler proof include all six original targets plus `streaming_work`. A regression-filter spelling correction was caught during preparation before any target ran; the earlier preparation and correction receipt are included. No build, regression, replay, or campaign failure was retried.

## Evidence and reconstruction

[Compact results](asan.json), [deterministic evidence archive](asan-evidence.tar.gz), [compressed member/hash/origin index](asan-evidence-members.json.gz), and [packager](package-streaming-policy-asan.py) accompany this report.

Archive SHA-256: `3bf0bbb5d9baf7f6cb7f7c60d959edb1bf47fe82862fae18ffce1ea9acc1dfca`. It contains 60,226 members, including 60,015 unique corpus blobs and 70 unpacked canonical source files. All members and corpus layouts were read back after compression. Seven compiled fuzz binaries are excluded with exact hashes; Cargo target caches and historical archives are not included.

`corpus-layouts.json` preserves every original filename for current initial/final corpora and historical initial/final seed inputs. Each value names its bytes at `corpus-by-sha256/<sha256>`. `corpus-origin-map.json` retains every checked source path for those bytes. This deduplicates bytes without losing corpus membership or provenance. Historical result metadata is explicitly scoped to seed provenance.

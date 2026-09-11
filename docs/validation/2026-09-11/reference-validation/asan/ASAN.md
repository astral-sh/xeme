# Selected reference-frame AddressSanitizer validation

Selected reference-frame source passed a fresh Rust AddressSanitizer build, 15 exact regression tests, retained-corpus replay and seven new 600-second fuzz campaigns. The saved records report zero failure artifacts, timeouts or target retries. An independent saved-data reader verified source, actual compiler vectors, representative machine code, exact outcomes, all fourteen replay/campaign commands and raw logs, and current and immediate historical corpus hashes. This is bounded sanitizer evidence; the project remains experimental.

Source commit `4064b0653534aff690c0e0f395a6b3b48da878fc` has the same 72 selected source hashes as measured runtime `0f66d54ac8418f0a6e628ad18570677a19c9ed45`. The broader 326-file source snapshot includes harnesses and all 246 committed seeds. Manifest SHA-256: `062832a888e9774cbd0dd17d4c0149afdd38ac4e00268dca995a04df485d8db7`.

## Outcomes

| Target | Replay executions | Campaign executed units | Final disk corpus files |
| --- | ---: | ---: | ---: |
| `ffi` | 9,250 | 1,834,341 | 2,310 |
| `ffi_family` | 10,763 | 481,293 | 922 |
| `multibyte` | 13,089 | 1,100,681 | 1,168 |
| `parse` | 9,567 | 3,080,949 | 2,753 |
| `streaming` | 12,221 | 1,463,416 | 1,849 |
| `streaming_work` | 12,293 | 1,422,066 | 1,835 |
| `value_family` | 4,640 | 556,669 | 667 |
| **Total** | **71,823** | **9,939,415** | **11,504** |

The 71,819 initial seed files are summed per target; identical bytes can appear in multiple targets. `streaming_work` retains its own previous initial and final corpus. Replay counts include libFuzzer initialization; campaign executed-unit counts separately include campaign initialization and mutation.

The six original harnesses are unchanged. `streaming_work` copies the streaming oracle with only `max_entity_expansion_bytes` reduced from 65,536 to 64 and `max_work_amplification: Some(100)` added. Thirteen policy regressions cover source/request bounds, positions, consumed-root credit, overflow, reset/old-child separation and namespace/default work. Two adapter tests cover decoded-reference ownership, accounting and callback/position/error boundaries.

## Instrumentation and bounds

Actual compiler vectors record ASan and coverage instrumentation for three workspace crates and seven fuzz targets. All seven binaries were rehashed and their ASan symbols read back. Retained representative disassembly shows ASan report calls in `XML_Parse`, core `parse_text` and `AdapterFrame::text_bytes`; this is a representative machine-code check.

Each campaign used `-max_total_time=600`, seed 20260911, maximum input 65,536 bytes, a 10-second per-input timeout and a 1,536 MiB libFuzzer RSS limit. Three lanes used CPUs 1, 2 and 4. Outer bounds were 180 seconds per replay, 690 seconds per campaign and 2,880 seconds for all campaigns; each exact regression had a 1,200-second bound. Controllers killed owned process groups on failure and waited for direct children. Saved first-attempt records and the completed root release support the outcome; the reviewer launched no parser/compiler/fuzzer targets.

`ASAN_OPTIONS=detect_leaks=0:abort_on_error=1`: leak detection was disabled. This does not validate LSan, UBSan, the entire standard library/libc, PGO builds, Expat differential equivalence or performance, and cannot prove memory safety or production readiness. Historical results are preserved as corpus ancestry, not transferred to the current source.

## Evidence and reconstruction

[Results](asan.json), [saved-record archive](asan-evidence.tar.xz), [member/hash/origin index](asan-evidence-members.json.gz) and [packager](package.py) accompany this report. The archive includes raw compiler/test/fuzzer records, source and exact harnesses, prerequisites, the intermediate build/regression review, the final saved-data reader and receipt, and corpus ancestry. Review stages state their own scopes.

Archive SHA-256: `73579260ee1df28286ea2561deb10a3cc0edfd17a34d02e16207b28c6c4d02b7`. All 72,087 members were read back after compression, including 71,469 unique corpus blobs and 326 source files. Seven current compiled fuzz binaries are excluded with hashes; Cargo target/cache trees and nested historical archives are excluded.

`corpus-layouts.json` maps every filename for current, streaming-policy, be22 and detached-frame initial/final corpora to its bytes at `corpus-by-sha256/<sha256>`. `corpus-origin-map.json` retains checked physical source paths; older detached-frame archive paths/names are preserved in ancestry metadata. Deduplication removes no logical corpus placements. The scripts retain historical absolute paths for this machine; archive members, source and corpus reconstruction are available without those paths. The archive separately retains the project licenses and upstream notices.

## Lossless publication compression

The published archive uses XZ to reduce the original 54,168,255-byte gzip archive to 12,797,748 bytes (12.20 MiB). This changes only the outer compression: all 272,291,840 uncompressed tar bytes, including every member, header and padding byte, are identical. Uncompressed SHA-256: `da9f0f2528ccf5ba6282761240f3abccd661644dc0fc5e7695ca5bf18894ac18`.

The retained [original packager](package.py) produced `asan-evidence.tar.gz`, SHA-256 `a9c5b323e1099b60dd47ba1837b719a3301e7269ae5e88913be45ca026d92efc`. The separate [converter](recompress.py) produced this XZ representation using single-threaded LZMA preset 9, a 64 MiB dictionary and CRC64. The [conversion receipt](recompression.json) preserves the original packet hashes and byte-for-byte tar binding. The outer member index points to the XZ archive; its 72,087 member rows are unchanged. References to gzip inside the preserved raw records describe that original package.

The original gzip package and its completed review receipts remain unchanged. The XZ archive was decompressed and all original members, source/corpus hashes and physical origins were checked with the adapted saved-data reader before publication. No compiler, parser, fuzzer or benchmark target ran during compression or readback.

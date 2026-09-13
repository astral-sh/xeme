# Independent PR116 package and workspace audit

The saved-data audit passed on its first attempt: **20,995 checks, 3,299 inspected files, and 3,270 recorded origin rows**. No parser, test, compiler, benchmark or packager was executed.

## Package integrity

- Main archive `bc63cf87`: 13,178,538 bytes, 1,863 members and 17 recorded binary exclusions.
- Prototype handoff: 1,038 members and eight exclusions; validation handoff: 332 members and 12 exclusions.
- Every direct and nested recorded member matches its current origin, recorded size and hash. Complete packaging selections, nested manifests and nested readback records were checked.
- Fifteen tar archive instances contain 4,073 member instances, at maximum depth two. Every member was read and hashed; no ELF or ar magic was encountered. This statement covers the visited tar member bytes, not arbitrary opaque formats.
- All nine top-level result/review copies match their original files.

## Source and saved workspace results

All 70 live source files and integrated source archives match runtime commit `be22a271`, parent `079fb66d`, manifest `735afb94`, and exact runtime patch `faf56975`. Historical prototype sources remain separately identified.

The saved workspace logs independently reconstruct **407 passing tests across 33 targets, plus one doc test**. Every target and test name matches the previous baseline inventory, with no added/removed/ignored/filtered tests. All four bounded commands completed with exit zero. Normal workspace test binaries remain distinct from the C-only libraries.

The outer documentation snapshot contains 15 files and no `files.json`. Later review attachments and the final index require a separate readback. This receipt does not certify future additions or perform the separate human prose review.

`generator.py`, `attempt-01.*`, all checks, full archive inventories, origin readbacks, copy hashes, source Git-read records and reconstructed workspace inventory retain the audit. `receipt.json` and `files.json` seal these review artifacts.

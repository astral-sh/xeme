# Keep detached callback storage within its reservation

A Text event can leave a 3,000-byte retained buffer. A subsequent valid 3,734-byte tag with 128 attributes needs 3,476 packed bytes, so ordinary amortized growth previously allocated 6,000 bytes. Together with the attribute records, the active frame retained 10,096 bytes against its 8,192-byte reservation on x86_64.

We keep ordinary growth when it fits the 4 KiB byte allowance. When an append needs growth from a capacity above 2 KiB, we reserve exactly to the 4 KiB ceiling once. The existing literal-tag predicate bounds packed fields by source-token length. Allocation failure still occurs before copying the field; value/name order, duplicate checks, raw publication and callback publication are unchanged.

The allocation tracker already counted the actual allocation. This fixes the separate frame retention allowance; the regression does not demonstrate an aggregate cache exceeding its complete 64 KiB budget.

## Validation

The same public parser/frame lifecycle regression fails before the fix and passes after it. All 438 core, C-adapter and storage tests (34 test groups, including doc tests), formatting and strict Clippy pass. An independent source review found no blocker and checked the retained red/green outcomes. No new speedup, Miri or external compatibility result is claimed here.

- [Failing baseline](red/red.log), [passing regression](green/green.log), and [full checks](checks-attempt01/report.json).
- [Independent source review](independent-review/README.md) and [review receipt](independent-review/review.json).
- [Source manifest](source.json), [check summary](summary.json), and [baseline regression patch](red.patch).

The reports retain exact commands, bounds, source hashes, exits and log hashes. Local Cargo uses Ohm with experimental defaults and trust disabled, a distinct target directory, a separate shared build directory, one job and CPU2. The 72-file source snapshot includes the inherited C integration fixture from PR #133; this fix changes only `crates/oriole/src/arena.rs`.

The regression can be rerun with `cargo +ohm -Zohm-defaults=no test --locked -p oriole start_after_text_stays_within_the_reserved_frame_capacity`.

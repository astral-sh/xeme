# Arena capacity fix: independent source review

**PASS: no source blocker found.** Reviewed patch `3778d7c1`, arena.rs `732da2c1`, manifest72 `857cca15`, base `ba0d5297`. All 72 live hashes match the freeze and Git's only working-tree delta is arena.rs. The inherited C integration fixture differs from selected source8a7d but is unchanged by this patch.

## Capacity proof

AdapterFrame::append is called only by the existing literal identity Start-frame lowering. That path requires a validated complete native root token of at most4096 bytes and at most128 attributes. Packed element/attribute names, values and their NULs fit within the serialized token: XML separators/quotes use at least as much space as the packed terminators. All intermediate append lengths therefore remain <=4096. Frame clear resets the byte length, and Text prepares exact bounded capacity.

- `capacity - len` is safe by the Vec invariant. The new branch runs only when the requested append actually exceeds spare capacity.
- For capacity<=2048, allocator-api2 0.2.21's actual amortized formula is max(2*capacity, required length, its small minimum). Every term is <=4096 for this byte vector, so the original growth is safe.
- For capacity>2048, a necessary growth instead reserves exactly `4096-len`. This subtraction is safe by the eligible packed-length invariant; it requests the full ceiling once, not a sequence of exact per-attribute increments.
- The following unchanged try_push_str and NUL calls fit the reservation, so their internal reserve checks cannot trigger another doubling. A sufficient-capacity append retains its original no-growth behavior.
- Attribute records remain separately reserved exactly for <=128 entries. The reusable Start arena now fits `4096 +128*sizeof(ArenaAttribute)`,8192 bytes on the measured x86_64 target. This is the existing reusable byte/record capacity accounting, not all parser memory or temporary detached End owners.

## Failure and lifetime behavior

Only the requested capacity policy changes. The reserve remains before the existing value/name byte writes and callback-byte assignment, after that attribute's duplicate check. Value-before-name processing, record insertion, final element-name copy, stack insertion, queued empty End, raw publication and final frame publication stay in the same order. Failed reserve keeps the previous Vec and initialized bytes; the selected allocator's existing failed-resize/tracker rollback applies. A failing tag remains unpublished, and ordinary parser/frame teardown releases owners through the same paths.

There is no new unsafe code, pointer exposure, callback borrow or allocator bypass. Exact request sizes/custom fail-at-N identity are intentionally not promised. The process-wide allocation tracker already charged actual allocations before this fix; this review does not claim a global allocation-limit bypass or prove a full64KiB aggregate-cache excess.

## Regression and retained outcomes

The regression uses the actual public Parser/frame delivery lifecycle. The first128-attribute tag warms lexical records, the second warms all128 detached records, and Text leaves an exact3000-byte byte-buffer capacity. The next validated3734-byte tag produces3476 packed bytes. Baseline doubling produced6000 byte capacity plus4096 record capacity: **10096 bytes against an8192 reservation**. The test checks active detached delivery, name, record count, packed length and actual capacities before releasing the frame.

The exact same test body (formatting only) failed on baseline and passed with the fix. The retained red exit101/log and green exit0/log, source pins,900second bounds and reaping are verified in source-readback.json. This is meaningful behavior coverage, not a synthetic call to reserve. The new test does not itself inject a low-memory failure; cleanup reasoning follows the unchanged fallible reserve/allocator path and existing allocation tests. Full core/C/storage/Clippy outcomes are author-owned and not claimed in this source receipt.

The saved-only reader read.py completed first attempt, session76403 exit0/reaped on CPU6. No target/compiler/profile execution or worktree edits by this reviewer.

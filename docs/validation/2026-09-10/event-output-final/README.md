# Owned event output integration

The core parser writes its next owned event into caller-provided adapter storage and returns the recycling token separately. This avoids an intermediate event copy. The existing public owned-event API remains available; the adapter ends its core borrow and DTD publication scope before invoking callbacks. Prefilled slots are cleared on entry, and errors or input exhaustion do not expose a stale event.

The root build passes 326 core/adapter Rust checks, strict Clippy and formatting. Both shared and static libraries are byte-identical to the independently tested author build, and all final source hashes match. Its exact-binary API, native sanitizer and differential evidence is retained through `reused-evidence.json`; no repeat test executions are implied. All 4,740 original API observations remain unchanged at 4,113 passing and 627 failing. Six native C sanitizer runs include 336 selected-allocation scenarios per linkage. These instrument the C consumers, not Rust, and disable LeakSanitizer. Root additionally reruns shared-table publication, active/DTD controls and 36,456 custom-encoding comparisons in isolated managed Python. Existing nine reference publication differences remain unchanged.

The independent review checks owned slot lifetimes, retained events, pending error prefixes, guard release, encoding conversions and zero-byte accounting. Its 15,030 parser invocations and 32 suspension/handler/input-window replays report no candidate/control changes. The shared CPython suite completes with only the two existing named fragmentation failures, followed by passing semantic checks; original failure logs remain visible.

## Native measurements

The original isolated seven-parse screen measured a 1.058× geometric project speedup. Root's short three-engine screen measured 1.037×, with two Batik conditions regressing. Because these tiny process batches changed sign between cohorts, a confirmation lengthens every small workload using counts fixed from the reference's earlier timings: Vulkan 7, Wayland 64, Maven 128, Batik 512, GTK/DocBook 256, generated controls 32. Five randomized cohorts use a separate fixed seed, one warmup and CPU 0. Every condition and every earlier sample is retained.

The longer confirmation measures a 1.023× geometric project speedup, with 22 of 24 conditions improving. Oriole still takes 2.54× Expat's time on average. These are native callback-driver measurements, not complete application execution. Complete normalized preflights and every sample hash/count agree; host load and CPU frequency remain uncontrolled. The smaller gain in this confirmation is the primary result.

## Actual consumers

Matched unmodified CPython modules parse the same six pinned project files. The selected library takes 1.71× Expat's time in ElementTree and 1.46× in pyexpat geometrically. Wayland through pyexpat takes 1.06×. All 24 consumer preflights and 120 timed processes pass complete output and library-origin checks. Five pairs each measure five parses after warmup, at 4 KiB chunks.

The unmodified Wayland scanner also produces byte-identical client headers and private code in all 280 measured processes. It takes 1.24× and 1.31× Expat's time respectively. These full process measurements include loading, input/output and code generation. Seven pairs each contain ten processes per engine/mode, after one warmup. Optional libxml DTD validation is disabled in both binaries.

The three scanner/accounting candidates excluded from this layer remain in the separate composition and attribution archive. No fresh sustained Rust sanitizer fuzzing, full W3C or PBS distribution campaign is claimed for this runtime. Those broader final campaigns remain necessary before a production readiness claim.

# Structural parser performance assessment

Status: read-only design; no runtime edits, prototype, new profiles, or performance claim.

## Recommendation

Keep lazy line/column positions as design evidence. Do not prototype that alone as the next large performance project. The measured position envelope is too small, and the equivalent reference work is substantial too.

The next useful structural prototype is **one shared tag plan feeding an owned adapter frame**, followed by the same frame mechanism for ordinary character data. Its purpose is to remove complete repeated grammar passes and intermediate owned-event preparation together. A plan that only skips another name check, changes an enum move, adds a pool, or defers positions does not meet this bar.

Start with native UTF-8, no namespace processing, no DTD/default attributes, no custom provenance, and ordinary literal values. Keep all other inputs on the existing semantic path. This is an attribution prototype, not sufficient coverage for production adoption: namespace mode and text must subsequently use the same representation before expecting a broad improvement. Successful baseline events and errors remain the oracle; original upstream tests remain unmodified.

The first code work is blocked only by the parent's requested freeze-validation boundary. There is no external approval requirement. Nothing here authorizes changing the active runtime or the selected PGO inputs.

## Evidence and realistic bounds

The inputs are the immutable 24 profiles in `/tmp/oriole-structural-profile`: six projects, namespace processing off/on, Oriole and Expat. Both engines perform one warmup and one measured parse under Callgrind, collecting only inside `XML_Parse`. These are instruction counts, not cycles or throughput. Native callbacks do not query line or column. Current Oriole parser-only instruction geomean, subtracting callback-driver instructions, is 2.43149 times Expat.

`opportunity.json` and `opportunity-table.md` contain the exact direct-cost reclassification and input hashes. `analyze.py` reproduces them without executing a parser. The percentages below use all collected `XML_Parse` instructions, including callback work.

- Oriole `consume`, `Source::position_at`, and `advance_long_position` together account for **6.50–10.56%**. This already includes required cursor/raw-index/scan reset/compaction work. Deleting every instruction in this envelope would give only about 1.07–1.12 times the current instruction throughput; doing so is impossible.
- Expat `normal_updatePosition` alone costs **11.68–21.09% of Expat's own instructions**, even without getters. In absolute counts, Oriole's broader position envelope is *smaller* on Wayland and Batik, and about 1.6–1.8 times Expat's count on the other projects. Percentages with different engine denominators cannot establish a position deficit.
- The disjoint token/name/XML-validation bucket is **13.78–17.83%**. It includes required work, end tags, DTD tokens, and character-data XML validation. The removable duplicate start-tag subset is smaller. Zeroing this entire bucket would still only give about 1.16–1.22 times current instruction throughput.
- C dispatch/loop, queues/recycling, String/Text helpers, and libc copies together account for **16.00–26.14%**. This is a broad, disjoint envelope, not removable overhead. It includes raw input copies, stack records and non-start events. The immediate `memcpy` edges from `next_event_inner`, `next_event_scoped`, and the pending-event queue are separated in the table; they alone do not justify an API redesign.
- Start callbacks are only about 23–30% of dispatch calls. A start-only prototype must not claim the full delivery envelope. Namespaces and common text are necessary follow-on coverage if this direction is selected.

The earlier fused completed-token validation/position experiment reduced instructions by 2.73% but slowed the 24 real-project timing conditions by about 1%. It is rejected and immutable. The new design replaces a representation and the grammar work that produces it; it does not repeat that experiment. Prior name-validation-only, enum-move, handler-snapshot, extra pool, and text-buffer-reuse studies remain rejected or unselected.

A plausible *research target*, not a prediction, is removing 12–18% of current instructions by reducing both duplicate lexical work and owned delivery preparation. That requires savings from both envelopes, with necessary work retained. A 20% instruction-throughput improvement needs 16.67% fewer instructions. Neither isolated envelope supports assuming this outcome. Even 25% improvement would leave a substantial native Expat gap.

## A. Lazy C positions: safe seam, insufficient priority

### What Expat actually does

The pinned primary source is `/home/dev-user/.cache/oriole/upstream/expat-2.8.4/expat/lib/xmlparse.c`.

- Around line 2392, `XML_Parse` updates position before retaining a non-final remainder. A successful final parse can return before this update.
- Around line 2510, `XML_ParseBuffer` folds consumed bytes and advances `m_positionPtr`; resume has a similar path.
- Lines 2777–2805 implement line/column getters by advancing from `m_positionPtr` to `m_eventPtr`, then caching that checkpoint.

Thus lazy coordinates primarily batch scans and sometimes avoid a final unqueried tail. They do not make coordinate tracking free.

### Proposed state and ownership

1. Keep byte index/count, raw-width accounting, resource charges, input consumption, and scanner state eager. Add a coordinate checkpoint describing a decoded offset, line, column, and preceding-CR state, separate from the consumed cursor.
2. Private event locations may be `Resolved(Position)` or a scalar source mark: source generation, decoded offset, byte index and byte count. Marks contain no pointers or references into mutable parser storage. Public owned `Event` and `Parser::position()` must always materialize exact `Position`; never use bogus line/column sentinels in public values.
3. The core must own the authoritative current adapter location. Merely copying a mark into `CParser.position` is unsafe logically: a later core consume may compact its backing bytes before the next getter. The adapter needs a central distinction between `CoreCurrent` and explicit resolved positions used for errors, custom conversions, and synthetic default fragments.
4. A line/column getter resolves and caches the core's current mark using a short nonallocating borrow, which ends before returning. The second getter at the same event reads the cached result. Per-event getters advance monotonically rather than rescanning from the start. Error lookahead must use a non-mutating position peek so it cannot advance the checkpoint past an earlier queued callback.
5. Before source compaction, source removal, encoding transition or parser reset, resolve every live mark whose backing bytes would disappear. Pending empty-element and namespace callbacks can hold earlier marks. Defer compaction until those pending events drain, or resolve their marks first; do not introduce an unbounded history buffer. Resolve the current event before discarding it even if parsing has already returned or is suspended.
6. Use decoded lexical source text for coordinate folds. Never use the adapter's roughly 1 KiB raw input-context window as a replacement, and never rerun a custom conversion callback from a getter. UTF-16/custom source raw widths remain independent. Anchored internal entities may use already-resolved anchor positions.
7. Initially activate only for a settled native UTF-8 root source. Flush before DTD/entity/custom/encoding transitions; external child creation and shared-table initialization must be audited, not inferred only from whether a DOCTYPE has been seen. Eager fallback remains exact.

This touches private location plumbing throughout emit/pop/error/pending paths, not just the getters. A larger internal enum can increase current 152-byte event transfers, so actual layout and caller-slot code generation would be an acceptance gate. The position opportunity is too small to justify this work as the next isolated prototype.

### Required correctness probes if revisited

No getters; only byte getters; one late coordinate getter; repeated line+column; a getter in every callback. Include split CRLF, Unicode columns, UTF-16/custom fallbacks, >64 KiB compaction, incomplete tokens, empty-tag and namespace queues, error-prefix callbacks, stop/resume, final queries, reset and parent/child lifecycles. DefaultCurrent and input context must remain exact. Getters cannot allocate or call a converter. Callback-time setters must not invalidate a borrowed Source because no Source borrow survives a callback.

## B. One incremental tag plan without allocations on incomplete input

### Existing seam

`Source::scan_token(ScanMode::Tag)` and `scan_element_tag` currently locate quotes, literal `<`, the terminator and token limits. After completion, `next_event_inner` copies the lexical token and validates XML characters. `parse_start` then calls `take_name`, QName validation, and `parse_raw_attributes`. Expansion subsequently scans values for references and whitespace.

The replacement is a single lexical state machine for **start tags only**: element name, inter-attribute whitespace, attribute name, before/after equals, opening quote, value, closing syntax. It yields a private `TagPlan` containing a scalar header and the existing `RawAttribute` offset records. `parse_start` consumes the plan and retains its current semantic order: depth/root checks, decoded identity, duplicate checks, defaults, namespace bindings, expansion, stack update, and event delivery. DTD and end-tag scanners remain separate.

The header can carry element-name range, empty-element flag, first invalid XML offset, first grammar failure, first invalid attribute QName index, and a tag-wide `all_values_literal_clean` bit. A tag-wide bit is enough for the initial no-DTD/native path; it avoids enlarging every 32-byte RawAttribute record just to skip a scan. Typed-default normalization and alias-sensitive values use existing expansion. Do not pack flags into supposedly unused offset bits: callers can configure large token limits.

### Avoiding early allocation

The parser already keeps up to 128 RawAttribute records (4 KiB). Use **only existing spare record capacity** during an incomplete scan. A capacity check must precede each push; there is no reserve, allocation, table lookup, entity expansion, or callback in this scanner.

- Cold tags with insufficient capacity continue the existing coarse scanner and completed-token parser. The completed parser warms the existing cache at its current allocation point.
- If a later tag exceeds spare capacity, abandon the partial plan, retain the existing quote/checked-offset state for boundary detection, and use the completed-token path. Never allocate just to keep the plan alive.
- Zero-attribute tags need only the scalar header. Very large attribute lists remain accepted under existing limits and are not retained beyond the existing cache bound.
- Token growth/reallocation changes no stored references: all records are offsets into the source's current token. Reset/source switch/mode switch/consume invalidate the header and record generation.

On a recorded grammar error or capacity overflow, preserve the coarse scanner's quote semantics exactly: that scanner recognizes quotes even in malformed syntax. Continue boundary/limit detection from equivalent checked-offset/quote state, rather than letting a failed grammar state choose a different `>` terminator. The exhaustive state oracle must cover this transition.

This is a shared scanner with an optional recording sink, not a second XML grammar. The completed-token fallback should eventually use the same lexical transition logic to populate records, while retaining its current time of allocation. The critical measurable difference from the rejected validated-name study is removal of the coarse-boundary-plus-full-grammar traversal on warm tags, and direct use of value classification in output preparation.

### Exact error and accounting constraints

Scanning may discover grammar/XML errors early but must **record**, not return, them early. The current scanner's literal `<`, token-limit and partial-input behavior have priority. For a completed token, retain the current sequence:

1. Charge consumed physical input at the current point.
2. Complete the fallible lexical token copy/current-raw ownership path.
3. Report the first invalid XML character in the whole token.
4. Apply root/depth checks, element-name/QName rules and decoded-name allocation.
5. Apply attribute grammar/limit checks, then per-attribute QName/duplicate/value processing in its existing order.

For example, an invalid control byte in a later attribute currently wins over an earlier malformed element name. A literal `<` at the limit currently wins over the limit. An incomplete malformed tag may remain pending until its terminator. The plan must preserve all three. No source/entity work can be precharged merely because the scanner looked ahead.

Fourth Edition NameRules and lexical representatives remain the grammar input. Raw ASCII colon has QName meaning; a custom converted colon does not acquire that meaning. Initial fast eligibility excludes provenance rather than attempting to reconstruct it. A future generalized plan must carry lexical slices through semantic projection, preserving raw name encoding identity.

### Why this alone is not the recommended performance project

The entire lexical bucket is only 13.78–17.83% and cannot be removed. Warm/cold eligibility further reduces coverage. The plan is justified as a common producer for a simpler adapter representation, not as an independent promised 15–25% improvement.

## C. Owned adapter frame instead of materialized common Events

### Problems to remove together

Current start delivery constructs one String for the event name and two Strings per attribute, emits a 152-byte Event through the pending queue, measures payload lengths again, appends NUL to every String, creates a pointer vector, calls C, then visits each owner again for recycling. Raw and expanded stack names have separate long-lived ownership. Pooling reduced malloc counts, but previous studies show malloc counts alone are a poor predictor of throughput.

The new frame should own **one contiguous C-string arena and offset records**. The tag plan provides source ranges; the existing semantic parser validates and writes the final callback spelling into the arena. No per-attribute String owner or terminator growth is needed. Stack names retain their independent ownership and exact raw identity. The original lexical token remains unmodified for DefaultCurrent.

The ordinary Rust API still returns owned Event/Attribute/String values by projecting this semantic result at its API boundary, or by selecting the existing owned builder. Grammar, duplicate detection, namespaces and expansion must be shared; only output storage differs. A storage abstraction must permit inspection by attribute index so current semantic ordering need not be rewritten to fit the arena.

### Smallest safe delivery seam

Use the existing caller-slot approach: a hidden adapter method fills caller-owned frame storage and returns a small delivery discriminator. Do not put a large new frame variant into EventKind or PendingEvent, add a heap box to every event, or rely on changing Rust return-value optimization. Measure real layouts and copies before broad benchmarking.

For the first native/no-namespace/no-DTD path, require an empty pending queue at the start of the token. After all semantic work succeeds, place the start frame directly into the output slot. An empty tag's end event remains queued and is delivered after the frame. All other cases use the existing owned Event path. If later extended to namespaces, pending namespace callbacks must retain independent owned data and their original order; the start frame must stay owned through suspension between those callbacks. Do not snapshot a handler before earlier queued callbacks can replace it.

Once the arena is finalized, the FFI constructs pointers into it. The arena and offset storage are owned local values outside the mutable parser for the entire callback. No reference into Source, token_scratch, a map, or CParser survives the callback. The existing busy and allocator-reentry guards remain in force. The local owners survive StopParser, ignored callback Free/Reset, setter calls, and external child parsing.

Large-attribute duplicate tables cannot retain `&str` keys into an arena that may grow. Use immutable raw-token ranges for native raw-name checks, or index/hash entries whose equality is resolved against the current arena only within a borrow. Preserve randomized hashing and bounded linear checks. Do not replace them with an unbounded quadratic scan or a collision-prone hash-only set. Namespace expanded-name duplication has the same constraint.

### Raw text and common character data

Do not mutate the raw token into NUL-delimited fields: XML_DefaultCurrent may request the original spelling inside the start callback, and ordinary current_raw remains public. The first arena prototype retains the existing raw owner; it does not claim zero token copies.

The same detached frame can later own ordinary character data and its raw spelling once, when they are identical (native UTF-8, no physical CR normalization or custom alias). This can remove the separate Text owner and public Event transfer rather than merely recycling a Text allocation. The adapter would expose the frame's immutable raw bytes to nested DefaultCurrent during the callback and restore the owned raw state before returning to parsing. Such a pointer is only a callback-scoped view into a separately owned local frame; clear it before dropping/moving the owner. Outside the callback, original core raw ownership must be restored. Non-identical raw/normalized text retains the existing two-owner path.

This extension is more consequential than the rejected text-slot pool, but its raw-lifetime state is also new. It is a second milestone, not something to hide inside the first prototype. It must preserve DefaultCurrent after parse/suspension, callback switching, converted output windows, coalescing boundaries and malformed-prefix delivery.

### Budgets and fallibility

Use the selected allocator for arena/records/pointers. Charge physical input, entity expansion and family callback bytes exactly as before; compact output storage is not permission to reduce work charges. Checked size arithmetic includes every NUL. Fallible growth must precede publication. A partial failure drops local owners and leaves no stale output slot, pointer, event, or generation token.

Any cached arena is part of the existing aggregate retained-memory budget, including record and pointer capacities. Avoid another independent 64 KiB allowance. No initial parser allocation is added; cold first-tag allocation, final retained memory, and peak memory are measured explicitly. The exact allocation schedule will change with the representation, so report changes to original allocation-retry tests rather than declaring those tests waived.

## Prototype gates after the freeze closes

1. **Plan oracle:** compare boundary/state/error/record results against existing scanning and parsing for short byte alphabets, arbitrary chunk cuts, limits, quotes, CRLF, non-ASCII names, Fourth Edition exclusions, incomplete input and custom-provenance fallback. No allocation occurs in incomplete recording mode. Measure warm-plan coverage on all six pinned projects, both namespace modes.
2. **Owned frame correctness:** baseline differential of successful callbacks and failures; existing 1,518 exact cases, original 4,740 API matrix, selected 36,456 custom cases, malformed tag oracle, allocator cutoffs, and native reentry/suspend/default/context probes. Source review must independently check the callback-local arena lifetime and every failure path. Public Rust owned-event tests remain required.
3. **Early performance gate:** compare code generation and disjoint Callgrind counts on Vulkan, Wayland and Maven, plus generated declarations as an unaffected control. The combined common-path prototype should remove at least roughly 8–10% of current parse instructions on two representative eligible projects without an instruction regression on controls before extending it. A few percent from another branch/layout change is insufficient. Track bytes copied and number of per-event owner preparations, not only mallocs.
4. **Broaden the same seam:** namespaces and common text must preserve the same semantics and ownership model. Stop if this requires a separate XML parser or unbounded retained token history. A broad candidate should target a measured 12–18% instruction reduction, with no claim that instruction savings guarantee time savings.
5. **Decision:** fixed longer-process paired timings on all 24 project conditions plus generated controls, then matched unmodified CPython consumers. Record original negative conditions; do not select only attribute-heavy inputs. Require a clear broad wall-time gain to justify the larger representation. If the combined seam cannot achieve that, reject it instead of following another chain of small pools and layout variants.

## Alternatives assessed

- **Borrowed Source events handed to C:** rejected as a design. Callback-time setters and child operations can borrow/mutate core state; retaining Rust references into Source or maps across callbacks is unsound. An independently owned arena/frame avoids that issue.
- **Move raw token to C and overwrite separators with NUL:** deferred. It can save arena copies but requires original-raw reconstruction for DefaultCurrent and changes a public core raw API. Start with the simpler exact raw owner.
- **Global interning of element/attribute names:** not the next prototype. It adds attacker-controlled retained state, collision/limit work, and a second lifetime model. Existing name owners are bounded and preserve custom raw identity. Revisit only with a concrete deduplication profile and bounded cache proof.
- **Generic direct callback sink borrowing `&mut Parser`:** rejected if the borrow crosses foreign code. A sink may only receive detached owned values after the parser borrow has ended, using the current output-slot seam.
- **Remove positions, budget tracking, duplicate checks, or token limits:** excluded. Those change correctness/security rather than eliminating redundant work.

## Evidence integrity

Original `/tmp/oriole-fused-validation` and `/tmp/oriole-structural-profile` remain untouched. This directory contains only this design, a read-only reclassification script and its outputs, source-reference hashes, and a manifest. It is not proof of independent execution or review of any future implementation.

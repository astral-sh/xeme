# Streaming work policy

## Behavior

The C interface permits cumulative source input up to
`min(c_long::MAX, isize::MAX)`, while retaining a 256 MiB maximum per input call or
buffer request. Its indirect-work and event-payload counters are now limited by
the greater of their initial allowance and **100 times consumed original root
bytes**. The initial allowances remain 8 MiB and 64 MiB respectively.

For example, repeatedly delivering a short namespace URI from a long XML stream
can continue after 8 MiB of cumulative namespace work. Reusing a very large URI
from tiny input still exhausts the relative allowance. These are work limits;
the existing 512 MiB limit on live/reserved family allocations remains independent.

The Rust `Limits.max_work_amplification` field is optional and defaults to `None`.
With `None`, `max_entity_expansion_bytes` retains its existing absolute meaning.
With `Some(factor)`, it becomes the initial allowance in
`max(max_entity_expansion_bytes, factor * consumed_root_bytes)`. The adapter uses
the same policy for its separate event-work counter. All other default Rust
limits remain unchanged, including the 256 MiB lifetime input limit.

The new work policy is independent of Expat's entity and allocation amplification
setters. Setting either Expat-compatible factor to infinity does not disable the
new fixed C work policy or the absolute live-allocation ceiling.

## Checked accounting and charge sites

`EntityBudget::work_limit` reads the same consumed direct-byte counter used by
entity amplification. It uses a saturating integer product, with no division.
Threshold saturation does not disable checked addition of either work counter.
The Rust `None` path returns the original absolute allowance without reading the
direct counter.

Every existing `charge_expansion` call remains at the same point before the
associated cloning, construction, or processing. Its atomic checked update now
compares against the input-relative threshold. The four C event charging sites
also remain unchanged in position and charged amount:

- Owned event dispatch, including raw declaration fragments and repeated base
  metadata.
- Start-element adapter frames, including names and attribute payloads.
- Text adapter frames.
- End-element adapter frames.

Absent handlers still incur the existing work charge. Split Default fragments
remain charged by their originating event; draining the pending queue after
suspension does not charge the same event again. No callback is invoked while
reading the budget or performing its checked counter update.

The C family's cumulative input count remains an overflow-checked statistic. It
does not grant work credit or subtract sibling history from a source's remaining
input allowance. `XML_Parse` checks source room and per-call size before changing
family statistics, allocation input credit, or input context. `XML_GetBuffer`
checks the same per-source room and per-request bound before allocation.

## No credit from future input

The allocation tracker's existing denominator credits accepted root-feed bytes.
The work policy intentionally does not use that counter. A whole input buffer
may have been copied while parsing is suspended at its opening tag; only the
recognized root prefix supplies work credit at that point. External input
remains indirect and never increases direct root credit. Accounting the same
token again produces no additional credit.

The suspension test feeds about 700 KiB at once and stops after `<r>`. The
expected work threshold with zero initial allowance is exactly 300 bytes, even
though allocator input credit already includes the complete feed. After resume,
the consumed bytes raise the threshold above 64 MiB. An old child can then charge
additional payload after its counter has been seeded at the former 64 MiB limit.

## Children, reset, and retained memory

The common C constructor selects the policy. Both external-child modes inherit
the configuration and share the original root's `EntityBudget`; each child has
its own cumulative source-input allowance. Root reset constructs a fresh core
budget and C family counter set. Children retained from the old document keep
the old budget and do not receive the new document's credit or reset their old
work charges. Child input, failed construction, and parent destruction do not
create new direct credit.

Allocation ownership and tracking are unchanged. Live backing bytes, pending
reservations, failed-allocation rollback, and blocks surviving parser destruction
retain their existing tracker rules. The 512 MiB ceiling, live allocation
amplification, entity-byte amplification, token/attribute/declaration limits,
cycles, external depth, and 1,024 child-construction bound remain enabled.

## Validation scope

The passing focused tests cover:

- Absolute Rust behavior versus opt-in relative work on repeated short namespace
  URIs and default attributes, using a small initial allowance.
- Large URI/default reuse that still exceeds 100 times consumed input, even with
  the independent Expat entity-amplification check disabled. A 64 KiB trailing
  whitespace suffix cannot finance earlier work, in either a whole feed or
  incremental feeds; rejection occurs before reaching the suffix.
- Saturated thresholds with overflow rejected by the charged counters.
- Direct/indirect credit, suspension, root reset, old children, and parent-free
  budget lifetime.
- Independent per-source input room, bounded buffer requests, and family-statistic
  overflow before source/storage mutation.
- The representable position and encoding cases in the arithmetic audit.

The complete policy passed formatting, all 420 workspace tests plus one doc test,
and strict workspace Clippy with all targets. The checks used Ohm with its
experimental defaults and trust flags disabled, one build job, and CPU 4. The
source manifest was identical before and after these checks:
`7595277315621876ca6fb8f1397631e2f2384d8c5c09fedfe7702c81cebb320b`.
The saved result is
`30d99cb3418d3ff5192c63e66282cc67fd7457a6a0b0120ed12f994fc06b022d`.
This status text was updated after testing; runtime and test files were unchanged.

The original prerequisite compile failure and two complete-policy Clippy failures
are retained. They concerned only test borrow syntax and configuration
initializers. Both earlier complete-policy workspace test runs also passed; no
runtime behavior or assertion changed to resolve those lint failures.

The [final report](README.md) records the completed 45-run stream comparison
through 257 MiB and the separate 2,049 MiB text/position probe. Original upstream
failure records remain unchanged; the two active upstream 2 GiB cases still hit
the original three-second bound. The standalone larger probe is separate evidence.
The final report also records shared/static CPython checks with the explicit
consumer cleanup backport: all 802 method outcomes match the earlier unmodified
consumer runs, retaining two failures per linkage. Performance remains pending.

## Arithmetic prerequisite

The source audit below uses accepted PR122
`1058868a7067671a1cb3b5f0de571e47cfcafe17`. Line references identify that accepted
source before the patch. Its Rust runtime is pinned by manifest
`1120c74e96c7f562cecfa25db227a830e8a1667a67dba30732ca039fb5e7dbec`.

### Entry-point invariant

For each parser's original source, let `R` be bytes accepted by `feed` and
`S = min(configured_total_limit, isize::MAX)`. Enforce `R <= S` before changing
`feed_start_byte`, `received`, finalization, the decoder, or input storage. A
limit error remains sticky; only error state changes on this rejection.

The new `input_bytes_remaining` query lets the C adapter perform the same
preflight before family accounting, allocator input credit, or raw input-context
mutation. `XML_GetBuffer` uses that allowance before reserving storage. The
existing C state/argument checks still take precedence. The 256 MiB single-call
gate precedes constructing a caller-provided byte slice.

Default Rust `Limits` remains 256 MiB. Explicit Rust configurations may permit
more cumulative bytes, but cannot exceed `isize::MAX`. The C constructor sets
its configured limit to `c_long::MAX`; the core applies the smaller bound. This
input policy and the relative work budgets above are both enabled in the final
implementation.

### Arithmetic audit

| Accepted-source operations | Bound and reason |
| --- | --- |
| Core `lib.rs:1154–1158`, `received + input.len()` and feed-start update | The new query checks the addition before input-state mutation; `R + len <= S <= isize::MAX`. The old code changed `feed_start_byte` before rejecting. |
| Decoder `encoding.rs:122,490`, BOM offsets; `:378–397`, declaration error position and line/column | The BOM consumes two/three accepted bytes. Declaration offsets are within its accepted pending input. Counting the BOM as one column still fits the number of accepted bytes. |
| Decoder `:154–175,207–244,529`, pending indexes, UTF-16 lookahead, converter width | A converter is scheduled only when its complete 2–4 bytes are pending. Every byte is advanced/drained once. `consumed + width` is within the pending slice; the `+1/+3` lookahead has headroom because the backing byte allocation is bounded by `isize::MAX`, and by tracked memory in C. |
| `:764–776`, multibyte `decoded_end` byte/line/column updates | Each successful conversion contributes exactly its original width once. `decoded_end.byte_index <= R`; decoded scalar count is at most original byte count. The source position used to initialize it includes any BOM. |
| `:31–38,790–807`, `raw_len`, UTF-16 sum/multiply, raw-width sums and cursor/range additions | Valid ranges stay within the live source view. UTF-16 units times two and custom widths equal bytes in the represented original range, so the result is at most `R`. Intermediate sums cannot exceed that result. Expanded UTF-8 storage may use more bytes, but its allocation remains independently bounded. |
| `:842,877`, direct and monotonic-cursor byte-position additions | The source raw prefix plus the range's original raw width is at most `R`. Monotonic position cursors cannot outlive consumption of their source; the existing ATTLIST callers obey that contract. |
| `:971,980,984`, accounting endpoint, `accounted_raw += bytes`, `raw_index += raw_len` | Account queries return only the still-unaccounted original prefix. Marking it does not add already-counted bytes. Both cumulative endpoints are at most `R`; the saturating subtraction does not need to conceal an overflowing addition. |
| `:985,992–999`, `cursor + count`, compaction and multibyte-width drain | Slice ranges and the cursor fit the current backing buffer. Compaction discards only stored prefixes and resets the buffer cursor; it does not reset cumulative original-byte or line history. Raw widths are drained using the same UTF-8 byte count. |
| `:1254–1291`, short/long line and column advancement | Every XML scalar consumes at least one original byte. A line count starts at one, so `line <= R + 1 <= isize::MAX + 1 <= usize::MAX`; a column is at most `R`. CRLF suppresses a line increment rather than increasing the bound. |
| Core `lib.rs:2757,2955`; DTD `dtd.rs:940,1034,1059,1110,1160`; semantic `semantic.rs:573` | These additions construct offsets inside the current token, declaration, or composed raw buffer, rather than adding lifetime source history. The unchanged token/work and live-allocation bounds apply. Source position projection either adds the original range within `R`, or returns an anchor unchanged. |
| C `lib.rs:2216`, `input_context_start += discard` | Discard is bounded by retained context length and by the earliest original byte still needed. Thus the new start is no later than the old captured input end. Extending context by a preflighted input keeps its end within the source's new `R`; allocation failure can leave a shorter window, not a larger endpoint. |
| C `:2250–2259`, relative context offset/size | Existing checked subtraction and length comparisons remain; conversions to `c_int` are fallible. A global input position is never cast to the context's local offset type. |
| C `:2134–2177`, byte, line, column and byte-count getters | The final `R <= min(c_long::MAX, isize::MAX)` policy ensures byte offsets fit `c_long` and `R + 1` fits the corresponding unsigned `c_ulong`. Byte counts still have their existing checked/saturating conversion; large individual requests remain bounded. |

Local scanner `index + delimiter_width`, lexical buffer append/range offsets, and
tag-scanner offsets remain relative to bounded backing buffers. No raw source
offset addition was found in the tag scanner: its original-byte start is an
identity used to recognize resumption. This review concerns source/position/input
context arithmetic; it does not widen token limits or claim to audit arbitrary
maximum-size Rust allocations in other subsystems.

### Source kinds and ownership

- **Root:** owns a fresh decoder/source and cumulative `received`. Its direct
  entity-accounting credit is separate from input storage and positions.
- **External DTD, value, and general-content children:** each constructor creates
  a fresh original source and `received = 0`, then inherits configuration. Each
  child's source obeys the same bound. Family counters and shared work bounds do
  not replace this per-source check; children can outlive the original parent.
- **Internal general/parameter entities:** `Source::entity` owns a finite UTF-8
  `String`. Its local raw history cannot exceed that backing byte length, which
  fits a valid allocation layout. `position`, `position_at`, and cursor
  projection return the referring `Position` unchanged; they never add the
  replacement offset to a potentially near-limit parent anchor.
- **Composed declarations:** temporary diagnostic sources use the same anchored
  constructor, sometimes with an empty local buffer. The original source is
  restored after parsing. Expanded declaration offsets do not increase the
  anchor's public byte/line values.
- **Reset:** constructs fresh source/input counters. Existing child sources keep
  their own coordinates and allocation owners. The patch changes no reset,
  child publication, callback, or allocator lifetime behavior.

The C live-allocation ceiling remains 512 MiB, including input buffers and
reservations. It protects simultaneous storage independently of the cumulative
source-coordinate proof. No new check was added to the per-character position
loop.

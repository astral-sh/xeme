# Bounded markup search for coalesced character data

## Proposed scope

Add one short fast path immediately before the existing scalar boundary selector in `Parser::parse_text`. When `coalesce` is true, search `<` and `&` together with the already available `memchr::memchr2` over at most the first 65,537 bytes. Return any found marker's index. If there is no marker and the whole span is at most 65,536 bytes, return the span length. In all other cases, run the original scalar selector unchanged. Noncoalesced prolog/epilog input also keeps that selector.

The 65,537-byte search includes index 65,536 deliberately. The original loop tests markup before its size/newline cutoff at that index. The search bound is a fixed semantic boundary from the current implementation, not a new tuning parameter. No parser source changes have been made for this proposal.

A future implementation should keep the shared existing fallback in one helper or closure; do not duplicate the later parser logic. It adds no parser state, allocation, new dependency, or public API. It does not change the cap, merge more text, or alter callback fragmentation.

## Why the shortcut is equivalent

For coalesced input, before index 65,536 the original loop can stop only at `<` or `&`. Physical newlines update a local `boundary` but cannot return it yet. Consequently, an earlier markup byte or exhaustion at length at most 65,536 determines the result independently of every intervening newline. At index 65,536, markup is still checked first; including that byte in the bounded search preserves its precedence over a previous complete line. If neither conclusion applies, the unchanged loop computes the result.

This proof also covers a cap inside a UTF-8 character: the search slice is bytes, and any returned marker is ASCII. Exhaustion returns the already valid source slice length. The cap itself is never returned merely because the search ended there. The fallback remains responsible for long lines and complete-line selection.

## Preserved boundaries and contracts

- The search receives exactly `remaining[..converted_text_limit]`, after the existing prolog literal/whitespace handling. It never scans into the next converter window or requests a conversion.
- CR, LF and CRLF semantics remain in the unchanged fallback and normalization. In particular, a CR at byte 65,535 followed by LF at 65,536 may produce a complete-line boundary at 65,537. An internal entity's literal CR is still ignored as a physical line boundary.
- The existing converted-window narrowing, trailing CR deferral, trailing `]]` retention, XML-character validation and forbidden `]]>` precedence run afterward at exactly the same selected `end`.
- Raw text saving, event/frame preparation, callback-byte charging, positions, ordinary Text publication before consume, sticky non-OOM error prefixes, and source accounting remain untouched.
- The extra work is bounded to one search of 65,537 bytes. If a long span falls back, the original loop runs once from the start. There is no retry state or repeated per-feed prefix recording. Even CR-only adversarial data adds at most that constant scan per original selector call; it cannot inspect beyond where the original selector might first stop without markup.

## Evidence and limits

`scalar-source.rs` is the exact selector and surrounding narrowing extracted from the fresh verified coalesced source archive. `inputs.json` records its archive/source/library hashes and the preserved fresh profiles. The pure Python oracle transcribes the original loop and models the first-marker contract with `bytes.find`; it is not a Rust parser or an implementation-performance test. Exhaustive short alphabets with small semantic caps exercise every ordering and relative boundary. Separate production-cap pairs, UTF-8 subspans/crossings and deterministic long spans exercise the real 65,536 boundary. Small caps are proof cases, not a performance sweep.

The fresh namespaced XML_Parse-only profiles report all `parse_text` self instructions as 3,840,664 / 27,728,788 (13.85%) on Wayland, 84,402,434 / 1,228,398,034 (6.87%) on Vulkan, and 111,208 / 2,969,194 (3.75%) on Batik. Those include setup, validation, normalization and output preparation as well as selection. They are upper bounds on removing the whole function's self cost, not isolated selector costs or predicted speedups. Earlier faithful-line 128-byte marker studies do not establish this candidate's performance.

## Approval and early gate requested

After source-level review and pure-selector equivalence, implement only this shortcut in an isolated worktree from the final selected coalesced source. Compare it with a fresh parent build using the same private intermediates and compiler settings. First require exact boundary/error/custom encoding probes, original API outcomes, and a Rust test comparing the actual helper against the unchanged loop. Then collect paired instruction counts on all six existing projects in both namespace modes, including the full-feed long-span cases and generated controls. Look for a material instruction reduction on Wayland and a second input, retaining every regression. Wall-time study and adoption remain separate decisions. Reject the direction if the new call/branch costs outweigh the removed scalar work.

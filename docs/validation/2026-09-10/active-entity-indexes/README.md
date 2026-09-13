# Active entity indexes on ebcd46d

This layer composes the previously reviewed active-source/value membership indexes onto `ebcd46dd0cd720d1f28de5fda0b87cf4b1b05756`, preserving the iterative attribute, NameRules, direct-PE, DOCTYPE, shared parameter-state, recycling and scanner changes. It then removes repeated inherited-name set construction from attribute expansion. It changes no default limits or C resource policy.

`candidate.patch` contains the final ten-file change against that Git base. `mechanical-control/` retains the nine-file mechanical composition; `inherited-only.patch` isolates the subsequent four-file attribute improvement. The original9470934 and7c0d8b35 handoffs remain unchanged.

## Behavior and ownership

Active and inherited general/parameter names use separate counted views in selected-allocator salted maps. The attribute helper consults only the inherited **general** view at its existing recursion check, alongside its local borrowed active set. It preserves full names (including a leading percent sign), lexical validation before recursion, recursion before depth/entity lookup, local frame push/pop, replacement provenance, and work charging. Current content-source membership is not added to attribute recursion.

Inherited membership is built once per child. Attribute expansion no longer copies the entire inherited chain for every shallow named reference. Empty maps allocate nothing for ordinary XML. Names are owned where callbacks can change tables; local borrowed sets stay inside callback-free immutable core operations. Existing external-child/table/context copying remains explicitly work-charged; this layer does not introduce shared DTD tables. Hash-map backing capacity can remain at the maximum active depth until reset/drop, while completed copied names are removed.

## Validation

- Release `liboriole_expat.so`: `c08038ff9bb9dc0e851c0ee3df30724a9c2764ee9418b31bf955594d7042fb92`.
- 68 exact frozen files, source manifest `fa19a62b849f101146f34ad2008c11bec3965514c1785d66fcaa595489fef632`; strict scoped Clippy and fmt pass.
- 339 ordinary Rust checks plus one compile-fail doctest pass. New coverage includes60,000 inherited names followed by10,000 two-reference attributes, subsequent reuse, full-name/error precedence, parent drop, salt changes, and failure at every selected allocation in a bounded workload. Initial fixture-only failures are preserved; runtime hashes did not change between those fixture corrections.
- All 4,740 original public API records exactly match ebcd46d:4,065 pass,675 fail, zero timeouts. Bounds remain3s/test,1GiB address space and768MiB RSS.
- 4,680 membership conditions (72 custom-encoding cases) have zero differences from the mechanical control. The360 conservative-depth outcomes differing from Expat remain; successful events match.
- 4,104 DOCTYPE conditions match reference exactly.1,944 missing-PE conditions retain24 error-code and162 event differences; four sibling and two general-context cases match reference.
- Six shared/static native C ASan/UBSan executions pass, with all selected allocations released. Rust is an uninstrumented release build and leak sanitizer is disabled. This is not a Rust sanitizer campaign.

The separate C100k-default experiment and measurement-only exports are **not** included in this patch or source archive. No new W3C, CPython, full Rust fuzz or throughput campaign is claimed for this layer.

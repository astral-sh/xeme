# Proposed Start-plan eligibility after DOCTYPE

Status: source-only proposal; no runtime edit, build or parser execution. Base is the 69-file final integrated snapshot `e05d0c4f`, library `9277b71c`. This is separate from the rejected atomic experiment and the persistent attribute-type proposal.

## Concrete scope

Replace the two historical exclusions `!seen_doctype` and `shared_tables.get().is_none()` in `Parser::next_event_inner` with a check that the **current scoped `tables.defaults` map is empty**. Retain all other Start-plan guards: start tag, no pending foreign DTD, non-fragment root parser, exactly one source, and `native_utf8_byte_index()`.

The resulting predicate is conceptually:

```text
start tag
&& !foreign_dtd
&& tables.defaults.is_empty()
&& !fragment
&& sources.len() == 1
&& native_utf8_byte_index is present
```

The existing control flow already dispatches DTD, external-subset, entity-value, header and declaration continuations before reaching this predicate. It also drains pending callbacks and foreign-DTD completion first. No new state flag, cached DTD summary, arena limit, scanner algorithm or namespace fast path is proposed.

Use the whole defaults map as the conservative first boundary. Do not narrow it to the current element name yet. In particular, an unrelated attribute declaration also retains the ordinary path in this first experiment.

## Why a default-free DTD is sufficient

`parse_start` consults DTD tables only through `tables.defaults`: for tokenized attribute normalization, omitted defaults and ID-attribute metadata. Entity references in attributes already force `TagScanner` to the ordinary path. General content references are dispatched before tag recognition. ENTITY, NOTATION or ELEMENT declarations therefore need not disable a literal tag plan by themselves.

The defaults map contains **all** successfully committed first attribute definitions, not just ones with literal default values. `attlist_attribute` inserts `#IMPLIED`, `#REQUIRED`, `#FIXED`, ID and tokenized declarations, with `value: None` where appropriate. Thus an empty map proves there is no declaration-driven normalization or ID metadata to skip. Empty `<!ATTLIST r>` contributes no definition and is safe; declarations suppressed by the existing skip policy do not become active definitions. Existing first-definition behavior remains untouched.

General-content children remain excluded by `fragment`; they keep their independent inherited snapshot. Parameter children are excluded by `fragment`/external-subset processing. However, a parameter child can publish definitions into the root's shared table. The predicate must inspect the **working table after the existing RAII scope has swapped in that owner**, never the parked empty table outside the scope.

## Publication, reentry and partial plans

- `next_event_with_tables` calls `with_dtd_tables`, which holds the allocation-free TryLock and swaps the current owner into the parser. `next_event_inner` runs inside that scope. The scope restores the table before any event/frame leaves the core. No table borrow or lock crosses C callback delivery.
- There is no C callback between reading the predicate and parsing the completed tag. A callback-created child can change shared defaults before the next core call; the next call rereads the current table. Current-parser reentry from selected allocator callbacks is rejected by the existing C busy guard; same-DTD parsing cannot acquire the already-held table lock. No new reentry mechanism is introduced.
- A nonfinal partial tag can leave a resumable plan. If an application parses a parameter child between feeds and adds an attribute definition, the next eligibility check fails and the original scanner handles that tag. `parse_raw_attributes` clears partial offset records before rebuilding them. `TagScanner` resets at a changed absolute source byte index; no prior plan is reused for the next token.
- Default definitions are not removed from a live family. Reset makes a fresh parser/scanner generation, and limits/salt setters do not make stale default definitions disappear. Still test the partial-plan/default-publication case explicitly.
- All pending DOCTYPE, external-reference, NotStandalone and EndDoctype callbacks retain priority. `finish_doctype` clears `in_doctype` and the pending foreign flag before normal content continues. An unresolved foreign request remains excluded by `!foreign_dtd` and the existing completion queue.

## Lexical and namespace boundaries retained

Native UTF-8 with no entity anchor is required by `native_utf8_byte_index`; UTF-16, Latin-1 and custom maps keep the ordinary path. Fragment parsers and nested entity sources remain excluded. The tag scanner still falls back for ampersands, physical TAB/CR/LF in attribute values, malformed names/tokens, exhausted existing offset capacity and token/attribute limits. It allocates no new record storage while collecting a plan.

QName validation and namespace binding/URI checks still run in `parse_start`. The frame's additional namespace-identity predicate remains unchanged: no prefix/default-namespace expansion, no namespace declaration attributes, and no colon-qualified attribute names. A completed plan can still use the ordinary owned event when namespace expansion is needed.

The token is still detached into `token_scratch`; raw publication, source accounting, consumption, error-prefix behavior and callback-byte charging remain in the same order. Start frames publish only after their existing fallible work. The current 4KiB/128-attribute arena and shared 64KiB retained-storage bound stay unchanged. No new lazy callback or unused-payload suppression is proposed.

## Evidence and expected reach

The current six real input files contain zero inline ATTLIST or ENTITY declarations; only Batik has a DOCTYPE. The established native driver does not load its external SVG DTD, so the broad DOCTYPE guard excludes Batik even though no attribute definitions are active. The source inventory finds 49 start tags, matching retained native sample metadata. Of those, 41 have literal attribute values without `&`/TAB/CR/LF and fit the existing arena size/count ceiling. This is a static upper bound: a cold offset cache can force additional fallback.

Batik installs a default SVG namespace. In namespace-on mode those tags still require ordinary namespace expansion, so the expected benefit is lexical planning only; no namespace-on arena claim is made. In plain mode the same tags may also use the existing arena. Eight multiline path values retain fallback. No wall-time or instruction improvement is established by this proposal.

## Required proof before any timing

1. Compare baseline/reference/candidate exact callbacks, positions, raw input context and `DefaultCurrent` for no DOCTYPE, empty internal subset, unresolved external subset, successfully read empty subset, entity-only/notation-only subsets and foreign DTD absent/read/skipped. Include both namespace modes and all relevant byte widths.
2. Confirm fallback for CDATA/ID/NMTOKENS/enumeration and `#IMPLIED`/`#REQUIRED`/`#FIXED` definitions, including a definition for an unrelated element. Check ID index, specified count, token normalization, duplicate first definitions, prefixed attributes/default namespace and namespace defaults.
3. Publish a new default from a callback-created parameter child before a start tag and between feeds of one partially planned tag. Also test creation without parsing, failed child parsing with earlier committed declarations, retained children, reset generations and handler replacement/suspend/resume.
4. Retain UTF-16/custom aliases/converters, internal and external entity sources, malformed tags/DOCTYPE suffixes, character/entity references, partial-token limits and expansion-limit/error-prefix controls. Verify unchanged error precedence.
5. Use the current allocation-failure sweep and exact original 4,740 API rows; report every changed outcome. Do not alter retry ceilings or add fictitious allocations. Record retained/peak capacity for first eligible tag and alternating fallback/frame tags.
6. First collect attributable Batik plain/namespace instruction counts against a fresh matched build. Only a confirmed improvement earns a later root-granted wall screen; include the five unaffected real projects as regression controls.

## Source anchors

- `crates/oriole/src/lib.rs:1465`: shared table scope before delivery.
- `crates/oriole/src/lib.rs:1873`: pending callbacks/continuations before content.
- `crates/oriole/src/lib.rs:2060`: eligibility predicate.
- `crates/oriole/src/lib.rs:2885`: unchanged arena/namespace filter.
- `crates/oriole/src/lib.rs:2975`: type normalization/default/ID uses.
- `crates/oriole/src/dtd.rs:1768`: all successful attribute definitions stored.
- `crates/oriole/src/tag.rs:267`: absolute-index reset and bounded fallback.
- `crates/oriole/src/dtd_tables.rs:67`: table scope ownership/restoration.
- `crates/oriole/src/encoding.rs:726`: native UTF-8 and anchor predicate.

Line numbers refer to the frozen snapshot identified in `identity.json`.

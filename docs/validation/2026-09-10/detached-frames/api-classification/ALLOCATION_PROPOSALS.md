# Remaining allocation costs: source-only assessment

Current final integrated API remains **4,347 pass / 393 fail**, across 4,740 original configurations. This report changes no outcome classification to a pass.

| Actual failing assertion category | Configurations |
|---|---:|
| Allocation/reallocation retry ceiling | 298 |
| Test requires at least one reallocation | 44 |
| Empty pre-initialization ParseBuffer expected to allocate | 12 |
| Fixed failure budget stops external child creation | 12 |
| Distinctive version string versus literal Expat identity | 12 |
| Absolute input cap | 2 |
| Absolute buffer cap | 12 |
| Input-buffer growth cost | 1 |

The 298 retry failures span 25 names. The complete 38 failing test bodies, original thresholds and every observed assertion are preserved separately. The 44 no-reallocation assertions are not opportunities to add needless allocations. A source optimization may fix a retry test, but only an unchanged full run can establish that.

## 1. Reuse omitted-default attribute buffers on repeated elements

This is the strongest **allocation** scaling opportunity identified from current source, separate from the post-DOCTYPE planning proposal. `parse_start` takes the bounded recycled attribute vector only when explicit attributes are nonempty, truncates it to that explicit count, then clones each omitted default name/value into a fresh record. A repeated `<e/>` using a nonempty default therefore neither takes the warm vector nor retains its default-field owners for reuse. A tag with explicit attributes also discards prior default slots before appending new clones.

Proposed bounded follow-up: determine the current element's declared defaults, detach the existing recycled vector when a default may actually be appended, and track the number of filled slots rather than truncating before defaults. Refill a returned slot's two original String owners using the existing `copy_attribute_string`; truncate only after all active attributes are filled. Preserve the original explicit-first/declaration-order sequence, specified flags, duplicate checks, namespace processing and ID index. Charge each reused default's name/value **before** copying exactly as today.

For a stable repeated element with D nonempty omitted defaults and adequate returned capacities, source inspection predicts up to **2D mallocs avoided per later start**, plus vector growth avoided. Empty default values do not currently allocate a value buffer, so the exact bound is D plus the number of nonempty values. This is not a measured current count or throughput result. If no event storage is returned, the ordinary owned Rust Event path must continue allocating; no shared references into the definition table may escape.

Keep the existing 128-record, 4KiB-per-string and aggregate 64KiB recycling limits. On the next callback, returned fields may have been expanded/rewritten, so refill by slot rather than treating previous contents as identity. Test alternating explicit/default counts, namespace expansion, large-to-small definitions, child-created defaults, default work limits, errors before publication and selected allocator reentry. No new cache is needed.

None of the six current real corpus files exercises DTD defaults. The three relevant unchanged failing examples (`test_alloc_attribute_enum_value`, `test_nsalloc_long_default_in_ext`, `test_alloc_long_attr_default_with_char_ref`, 12 configurations each) apply their default once, so warm reuse alone should not be advertised as fixing them. A real document/consumer with repeatedly applied defaults must be identified before claiming a real-workload improvement.

## 2. Store attribute semantics without retaining the full type string

`DefaultAttribute.attribute_type` is a heap String, cloned from the completed semantic type and cloned again in a general child's independent snapshot. Its later uses are only `!= "CDATA"`, `== "ID"`, and snapshot work charging of its length. Store a scalar kind (`CData`, `Id`, `Other`) plus the original normalized type byte length instead. Keep the complete and partially captured **callback** type Strings unchanged, preserving handler-mutation payloads and family callback charges.

This removes one persistent nonempty String allocation per first attribute definition and per general-child snapshot copy. The saved ac6a/4b enum trace has the corresponding 66-byte selected allocation at parse-allocation index44, followed by separate default-value and index-key clones. That is historical site attribution, not a final9277 count. Snapshot accounting also charges `size_of::<DefaultAttribute>()`; shrinking the physical descriptor would change that charge unless the prior logical overhead is deliberately preserved. A concrete patch must explicitly preserve both the original type length charge and that existing overhead, or document a separately reviewed policy change. No silent cap weakening.

This is a small retained-memory proposal, not enough to bridge the saved enum cost of58 mallocs versus an exclusive retry ceiling30. It has no demonstrated effect on the six real corpus conditions.

## 3. Remove the duplicate attribute-name owner in the lookup index

`DefaultAttributes` owns one name in `ordered` and clones another into `by_name: HashMap<String, usize>`. A salted numeric-index table comparing `ordered[index].name` can remove one malloc/name copy per first definition and snapshot. The saved enum trace identifies a separate57-byte key copy at index46. Preserve reserve-before-commit transactionality, stable declaration order, collision resistance, salt replacement and clone behavior. Do not introduce borrowed keys pointing into a movable vector.

This is a bounded memory improvement but broadens the hash-table implementation surface. It remains lower priority than default-owner reuse and has no measured real-input gain. The old snapshot budget charges the duplicated key's descriptor/name; preserving those logical charges is a separate explicit requirement.

## 4. Larger DTD pool/descriptor ownership remains a design project

Historical successful traces show notation35 mallocs versus Expat8–9, enum58 versus21. Token/raw reuse already reduced notation35→31 and enum58→55 without a useful wall gain and increased retained scratch; that prototype remains rejected. The previous unused-entity-payload study similarly removed several allocations without improving original API assertions or real-project throughput, and an initial move retained oversized capacity.

A shared immutable completed definition or pooled DTD callback descriptor could remove multiple name/type/value/Box copies, but must handle general-child snapshots, shared parameter publication, unfinished reserved identifiers, sticky `value_open`, callback-dependent enumeration captures, lexical aliases and exact charge/error ordering. It is a larger lifecycle redesign. Current evidence does not justify choosing it merely to approach an incidental retry ceiling.

Replacing only persistent short DTD names with inline immutable owners is also bounded in scope but cannot address the 1KiB notation/name tests; long names still allocate. Do not modify global String layout or its `into_bytes` contract to pursue this.

## Recommended sequence

First assess the separately specified post-DOCTYPE Start-plan seam on Batik, where a real input demonstrably misses the selected optimization. Then, if allocation work remains the priority, profile a real repeated-default consumer and prototype reuse of the existing returned default-field owners. Keep scalar type/index reductions as independent memory layers. No runtime work or new parser/profile/timing run was performed for this assessment.

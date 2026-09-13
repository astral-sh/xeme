# C entity defaults for deep Expat workloads

The C constructors now allow 100,000 entity declarations and 100,000 entity nesting levels. The safe Rust defaults remain 10,000 declarations and 32 levels. This policy layer follows the iterative attribute expansion and active-name index changes; it contains only the C constructor configuration, focused tests, and C README.

All other limits remain unchanged: 8 MiB of expansion work, 512 MiB of parser-family live allocation, 256 MiB of family input, 16 MiB tokens, 256 element levels, 10,000 attributes, 64 MiB of callback payloads, 1,024 child-creation attempts, and 32 C external-child ancestry levels. Relative expansion and allocation guards remain enabled. Reset retains the C defaults; children inherit them.

## Why change the C defaults?

The larger limits let existing Expat callers parse the original deep-entity fixtures without using a nonstandard setter. An opt-in setter alone would leave those callers failing until their integrations changed. Iterative expansion and indexed active names remove the host-recursion and repeated membership scans that previously made deep inputs unsuitable.

This admits higher resource use for documents that previously stopped at the smaller limits. The selected-allocation maximum below is a measurement of these fixtures, not a bound for every document with 100,000 entities. The unchanged 512 MiB family ceiling remains the absolute allocation policy.

## Exact original API result

The original 4,740 records change from **4,065 pass / 675 fail** to **4,101 pass / 639 fail**: exactly 36 failures become passes, with no other record changes and no timeouts. These are the unmodified 60,000-level content, 60,000-level attribute, and 70,000-level delayed-parameter fixtures across all six chunk sizes and both deferral modes. The delayed fixture declares 70,002 entities in total.

The original test bodies, constructors, assertions and bounds remain unchanged: 3 seconds per case, 1 GiB address space, and a requested 768 MiB RSS limit. The instrumented measurement helper is separate from this gate.

## Supplemental measurements

Each row covers 12 configurations on CPU 3 of a shared host. Times are one sample per configuration, not a statistical throughput comparison.

| Original fixture | XML bytes | Largest selected allocation | Charged work | Slowest final parse |
| --- | ---: | ---: | ---: | ---: |
| Content, 60,000 entities | 1,777,818 | 102,892,503 B | 468,890 B | 0.662 s |
| Attribute, 60,000 entities | 1,777,834 | 70,018,905 B | 468,890 B | 0.498 s |
| Delayed parameter value, 70,002 declarations | 2,497,866 | 77,626,792 B | 5,647,803 B | 0.855 s |

Selected allocation counts bytes requested from the chosen suite, including Oriole's tracking headers. It excludes the harness's own header and the separately recorded host input buffer. Peak RSS is observed separately; Linux `RLIMIT_RSS` is not treated as a proven hard resident-set limit. Address-space and wall-clock bounds are enforced by the controller.

Work counters were read in a separately hashed experiment with two allocation-free, atomic-read-only diagnostic exports. Those exports are absent from this production source. All 36 production observations exactly match the instrumented experiment's status, semantic results, allocation counts and selected peaks. A zero work field in production helper output means unmeasured.

## Validation

- 342 ordinary scoped Rust checks plus one compile-fail doctest; strict Clippy, formatting and release build pass.
- Constructor/reset and both child-mode tests cross the old count and depth limits and verify semantic output. Count 100,000 succeeds and 100,001 fails. A manually created parameter child accepts total depth 100,000 and rejects 100,001 without hitting the count cap first. Real root input supplies the relative allocation denominator; no guards are disabled.
- Cycles, 100,000 repeated empty references, lowered work limits, and the existing family/input/allocation guards remain covered.
- Six shared/static native C ASan/UBSan executions pass. Rust is an uninstrumented release build; leak sanitizer is disabled.
- On the final library, 36 successful selected-allocator observations and 152 injected locations leave zero live bytes or blocks. The latter comprise 149 actual `NoMemory` failures and three successful controls beyond the last allocation. This sample is not an exhaustive sweep through the hundreds of thousands of deep-fixture allocations.

Final library: `390071e398a62809dbbc5443e46ad1f574a8364647b42ed475dee6829ba60a53`. Source manifest: `6ee4b4313cb09700d64b708567e441b15453a2541bf0b3901e21706cefd5046d`.

## Remaining resource cases

The 12 original large-buffer configurations also fail in pinned reference Expat under the **same current 1 GiB address-space bound**, at the required non-null `XML_GetBuffer` assertion. The first prefix is empty, so its first request is exactly 1,073,741,824 bytes before parser and process overhead. These current rows are a shared harness-bound limitation. Separately, Oriole's unchanged 256 MiB input-reservation and 512 MiB live policies reject that request even with a larger process allowance. The earlier reference matrix used a 4 GiB address-space allowance; it is not evidence that this buffer fits the present bound.

Two active configurations of the original two-GiB streaming test remain constrained by the 256 MiB family-input budget. That fixture feeds 2,147,483,675 bytes; its other ten chunk configurations return early. Changing total-input policy requires a separate decision and throughput/counter validation. Neither group is relabeled as a pass here.

# Unique-owner accounting prototype — unselected

The prototype preserves the reviewed accounting behavior, but its measured native wall time does not support adoption. Across the 24 real-project conditions, combined/baseline time is 1.010209 geometrically: 20 conditions are slower and four faster. All 12 plain-mode conditions are slower. The four generated controls average 1.012451; all four retained-child controls are slower, averaging 1.017777. Every planned sample is retained. The parser source remains isolated from the main checkout.

## Change and ownership proof

A new `Shared::get_mut` checks uniqueness through an exclusive handle and an Acquire reference-count load before forming a mutable reference to the value field. This proof depends on the current strong-owner-only API: there are no Weak owners or raw-owner constructors. Uniqueness is checked afresh and never cached across callbacks.

Only core `account_source` and the three C callback-payload charges use this path. The allocation tracker, other expansion charges and input/child accounting are unchanged. No default, quota, counter definition, parser field or allocation is added. Core successful increments remain stored before later total/relative rejection; C rejected charges remain transactional. Core zero-byte work skips the ownership probe, whereas C zero-byte charges still check the existing total against the cap.

Ordinary Text retains its established publish-before-consume error prefix on non-NoMemory failures. CDATA retains consume-before-publication. All exclusive budget references end before callback or failure dispatch. Independent primitive, final-source, lifecycle and test reviews are in `reviews/` and `first-runtime/`.

## Validation scopes

- Original default-Ohm combined artifact `195db6a3` (static `7e91f7c7`): 391 workspace all-target release tests, strict Clippy and formatting pass. Six tests are new; one existing Text/CDATA error-prefix test is expanded.
- That exact artifact matches all 4,740 original API result rows against the canonical e439 baseline: 4,323 passes and 417 retained failures. Assertions and 3-second / 1GiB address / 768MiB RSS / 240-second total limits are unchanged.
- Six native C consumers pass with ASan/UBSan, shared and static; each allocation sweep covers 327 scenarios. Rust release libraries are not sanitizer-instrumented; LeakSanitizer is disabled.
- Fresh no-default builds use the root-verified intermediate directory template containing `{workspace-path-hash}`, empty compiler wrappers/flags and three full recorded workspace compiler invocations per variant. Matched baseline `16dd25dd`, combined `938f6f5f`, core-only `1ead4ce0`, callback-only `2287f27d` are distinct from the original default-mode gate artifacts.
- Old-to-fresh baseline and combined controls match in 4,816 strict comparisons: 2,392 malformed/text/event/position cases plus 16 streaming suspension/DefaultCurrent pairs for each old/fresh pair. These do not constitute a second full API run on the fresh libraries.

## Performance evidence

Forty untimed preflights and 40 Callgrind processes cover Vulkan, Wayland, Batik and the existing generated entity fixture, each with unique and retained-child owners. Each process parses twice. Collection is restricted to `XML_Parse`, including callbacks. Matched-build combined instruction counts are essentially flat for unique owners and increase about 0.10–0.30% for shared owners. Core-only instruction reductions and callback-only increases largely cancel. Original default-mode e439 is retained only as a diagnostic fifth engine. Instruction counts do not measure locked-operation latency.

The separately reviewed wall protocol uses the matched fresh baseline and combined libraries. It covers six real inputs in both namespace modes at 4/64KiB, two existing generated controls at both sizes, and four retained-child controls at 4KiB. Seven randomized pairs produce 448 workers, 78,890 measured parses and 448 excluded warmups. All 120 preflight workers pass: 64 selected-driver cases plus 56 ordinary-driver equivalence controls. The timed driver includes parser construction, parsing, native callbacks and destruction; retained-child times also include child construction/free. This is native lifecycle timing on a shared host, not application timing or an Expat comparison.

The driver hashes selected normalized callback metadata and does not independently prove exact text fragmentation. The strict cross-build probes cover complete events and positions separately. Retaining a manually created empty-context child is an Oriole-only ownership control, not an external-DTD throughput claim.

## Preserved development and tooling evidence

Initial test compilation and Clippy failures concerned new assertion borrow syntax and are retained. The first API postprocessor rejected output-directory paths and adapter-directory inventory differences after the successful parser run; the corrected comparison verifies actual used sources, configuration, selection, bounds and every result row without rerunning the API suite.

The earlier wall coordinator/preflights are preserved. The final coordinator additionally kills/reaps a worker group on interruption and records launch failures; controlled negative tests preserve partial output and verify cleanup. Timing began only after independent review and the explicit CPU0 lease, and the controller completed with exit zero before release.

The root's stale no-default build discovery is documented separately in the retained build provenance. No original library or shared cache was deleted or overwritten by this task. Fresh-build attribution uses verified compiler invocations and matching build mode. All executable libraries/drivers are omitted from the handoff archive; their exact hashes, source archives, commands, reports and raw outputs are retained.

# Reviewing Oriole

Start with the [current compatibility contract](compatibility.md), then the
implementation layer relevant to the change. Historical qualification belongs in
[the evidence index](evidence/README.md); it does not automatically qualify a new
commit.

## Architecture

| Layer | Main responsibilities | Review focus |
| --- | --- | --- |
| [`oriole`](../crates/oriole/src/lib.rs) | Safe streaming parser, encodings, namespaces, DTD and owned events | Progress across partial input, grammar/error order, bounded expansion, source positions |
| [`oriole_storage`](../crates/oriole_storage/src/lib.rs) | Fallible allocator-aware storage | Allocation ownership, failure cleanup, shared budgets and overflow |
| [`oriole_expat`](../crates/oriole_expat/src/lib.rs) | C ABI and callbacks | Pointer validity, detached callback storage, handler mutation, reentry, reset and child lifetime |
| [`oriole_cli`](../crates/oriole_cli/src/main.rs) | CLI input and event output | Input errors, resource defaults and allocator selection |

Parser fast paths and owned fallbacks share grammar and semantic helpers. Keep
allocation timing and callback accounting explicit when extracting shared code:
removing repetition must preserve suspension, callback mutation, sticky errors and
caller-selected allocation. Avoid expanding one optimization into unrelated
representation changes.

## Reviewing a change

1. Reproduce the concrete behavior with a small input and the reference parser.
   Separate acceptance, callback payload, fragmentation, position and resource policy.
2. Review both success and failure paths. Include partial input, empty input,
   allocation failure, reset and external-child lifetime where relevant.
3. Run the relevant regression plus the continuous compatibility gates. Keep
   upstream failures in raw results; do not loosen an allowance by method name.
4. For a performance change, freeze the emitted libraries, validate equivalent
   outputs, and measure parent, candidate and Expat in matched rounds. Retain
   regressions and negative experiments. Use the [iteration guide](../benchmarks/HILLCLIMB.md).
5. Bind evidence to source, toolchain, input and artifact hashes. Keep compact
   summaries in the repository and large raw workers in durable artifact storage.

## Adversarial review follow-up

The September 2026 review found reproducible defects in DTD import accounting,
benchmark artifact selection, ABI metadata and test selection. The follow-up stack
adds recipient-side accounting, emitted-artifact binding, exact failure and
inventory checks, complete allocation selector coverage, an isolated Expat fuzz
oracle and continuous API/W3C/differential regression gates. It also removes the
personal PBS branch gate, shares duplicated semantic/dispatch helpers, and
separates a new performance holdout from the six projects used for tuning.

Evidence cleanup removes large historical files from the current tree without
rewriting history. [Archive manifests](evidence/archive-manifest.json) preserve
hashes, original source identities and a correction to two historical fuzz counts.
Current resource policy has one authoritative description in the compatibility
guide; individual experiment narratives remain historical.

## Interpreting green checks

A passing regression gate says that the reviewed boundary has not regressed. It
is not a passing upstream conformance suite. The [known differences](compatibility.md)
include allocation schedules, resource policies, strict CPython fragmentation and
W3C name-profile expectations. A test blocked by an early assertion does not cover
its later assertions.

Similarly, sanitizer and fuzz results describe the executed harnesses, inputs,
bounds and source. C sanitizers linked to uninstrumented Rust do not instrument
the Rust implementation. Report initialization, replay and mutation counts
separately. Installed-distribution validation and performance are separate from
local shared-library testing.

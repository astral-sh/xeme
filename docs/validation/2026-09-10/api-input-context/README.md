# Input context and public memory helpers

This checkpoint implements `XML_GetInputContext`, preserves the original encoded
input around pending events, and reports the initial byte index as `-1`. Public
`XML_MemMalloc` and `XML_MemRealloc` allocations now follow Expat's application-owned
allocation contract: they use the selected memory suite but bypass parser memory
amplification accounting, including calls from an active parsing callback.
Parser-owned storage, callback buffers, children and content models remain tracked.

The candidate builds on `41616ed`; its release shared library has SHA-256
`c5d35ae574d513dde3bb0f4298e90ada6f794ae58a8db3d9594351654e3e6533`.
[The report](report.json) records all Rust source hashes, manifests, toolchain,
commands and library hashes. [Evidence](evidence.tar.gz) contains the full adapted
upstream sources, observations, command logs, native runs and separate allocation
diagnosis. The archive includes its own per-file hash manifest;
[SHA256SUMS](SHA256SUMS) identifies the outer artifacts.

## Compatibility results

The unchanged Expat 2.8.4 public suite completes all 4,740 configurations, with
**3,751 passes and 989 failures**, compared with the prior 3,753/987 checkpoint.
Every failure is an assertion exit; no configuration crashes or times out.

The change fixes 48 functional configurations:

| Contract | Configurations fixed |
| --- | ---: |
| Raw input context for character data and internal entity references | 24 |
| Initial and final byte-index observations | 12 |
| Public memory helpers under restrictive parser memory limits | 12 |

Two additional allocation-schedule configurations start passing. The raw suite
also gains 52 allocation-test failures: the additional context storage exceeds
those tests' fixed malloc/realloc retry ceilings. They cover five tests, rather
than 52 different API features.

A **separate diagnostic replay** raises the existing retry ceilings to 512 while
preserving all assertions. All 52 newly failing configurations then pass. Across
the five selected tests and all 12 contexts, this replay has 56 passes and four
failures. The remaining four were already failing before this change and require
a realloc call that Oriole does not need for their whole-input configuration.
These diagnostic outcomes do not replace the 989 failures in the unchanged suite.
No previously passing non-allocation configuration regresses.

## Ownership and bounds

The context contains original bytes, including UTF-16 and original entity-reference
spellings. It retains 1,024 bytes before the current pending position plus pending
and newly supplied input. Storage belongs to the parser's selected allocator and
remains subject to its memory budget. The returned pointer is available only during
active parsing and is valid for the requesting callback. Reset and destruction
callbacks cannot obtain an old context.

Regression tests exercise direct and buffer input, one-byte feeds, long split
attributes, internal entities, UTF-8/UTF-16, suspension/resumption and reset. All
66 FFI unit tests and the selected-allocator/reentry integration test pass, as do
formatting and strict Clippy.

All six shared/static native integration, adversarial and allocation runs pass
under C address/undefined sanitizers. The allocation sweep covers 358 injected
failure scenarios for each linkage. Adversarial callbacks retain and reread the
context across callback-safe mutations and independent nested parsing, and use
public memory helpers under a zero parser-tracker threshold. These runs use an
uninstrumented release Rust library; leak sanitizer is disabled. The first native
compile attempt lacked the test's `XML_GE` declaration switch; its failed build
log is retained separately.

This checkpoint does not settle the other compatibility gaps, exact callback or
position differences, the remaining allocation assumptions, or performance.
The extra raw-input copy belongs in subsequent combined benchmarks.

## Retention follow-up

Independent review found that using the last delivered event's position retained
an entire eventless whitespace prefix. A 1 MiB prolog supplied in 1 KiB chunks left
1,048,580 context bytes at the root element. The corrected parser exposes the
earliest input position still referenced by a source, queued event or pending
DTD/value continuation. The adapter discards consumed prefix bytes while retaining
those origins. The same probe now retains 1,028 bytes, exactly matching Expat.

The corrected shared library is
`474ebbb65712a466890ab05cf17db489ad4e4346de385dd5a7532d474281b7f4`.
All 224 core/FFI checks pass, including eventless prolog/DTD whitespace and long
split declarations with parameter entities and optional default callbacks.
Strict Clippy, formatting and all six native sanitizer runs also pass. The full
upstream suite has the same 3,751 passes and 989 failures as the first layer;
every individual outcome and adapted test-source hash is unchanged.

[Retention report](retention-report.json) and
[retention evidence](retention-evidence.tar.gz) preserve the reproducer,
independent review, source hashes, full upstream observations and native/gate
logs. The initial over-retaining candidate and its evidence remain recorded.

## Integration after the throughput changes

The compatibility fixes were replayed onto the first event/buffer throughput layer. All 225 core/FFI checks and six C ASan/UBSan runs pass, including 349 selected-allocation failure scenarios per linkage. The lower allocation count reflects the earlier throughput changes. Rust is uninstrumented in these native C runs, and leak detection is disabled. [integrated-throughput.json](integrated-throughput.json) records the source/library hashes, commands and native output; [the compressed test log](integrated-throughput-tests.log.gz) retains the Rust results. The upstream matrix counts above describe the isolated compatibility layer, before throughput integration.

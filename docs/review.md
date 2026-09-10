# Reviewing Oriole

The implementation is published in native GitHub Stack 3, starting at [PR #1](https://github.com/astral-sh/oriole/pull/1). Each layer uses the preceding layer as its base. The PRs remain drafts for review; none has been merged or released.

## Review order

| Area | Layers | Main questions |
| --- | --- | --- |
| Parser, C interface and storage | [Foundation](https://github.com/astral-sh/oriole/pull/1) through [hardening](https://github.com/astral-sh/oriole/pull/9) | Owned streaming events, selected allocation, input boundaries and callback ownership |
| Initial performance work | [Benchmarks](https://github.com/astral-sh/oriole/pull/10) through [namespace names](https://github.com/astral-sh/oriole/pull/14) | Equivalent outputs, parent/candidate measurements and allocation costs |
| Consumer and lifetime contracts | [Consumer contracts](https://github.com/astral-sh/oriole/pull/15) through [family fuzzing](https://github.com/astral-sh/oriole/pull/24) | CPython integration, allocation failure, parser-family lifetime, reset and reentry |
| Distribution and lexical compatibility | [PBS distribution](https://github.com/astral-sh/oriole/pull/25) through [namespace measurements](https://github.com/astral-sh/oriole/pull/35) | Actual distribution builds, encoding, DTD boundaries and incremental scanning |
| Callback and external-value behavior | [Callback allocation](https://github.com/astral-sh/oriole/pull/36) through [value-family fuzzing](https://github.com/astral-sh/oriole/pull/46) | Handler mutation, declaration visibility, child ownership and continuation state |
| Conformance and final grammar | [Combined measurements](https://github.com/astral-sh/oriole/pull/47) through [grammar fuzzing](https://github.com/astral-sh/oriole/pull/61) | W3C acceptance, namespaces, versions, foreign DTD policy and external declaration grammar |
| Final evidence | [Validation and benchmarks](https://github.com/astral-sh/oriole/pull/62), [full PBS gate](https://github.com/astral-sh/oriole/pull/60) | Source identity across consumers, fuzzing, benchmarks and installed-distribution validation |

The [final runtime report](validation/2026-09-10/final-runtime/) and [benchmarks](../benchmarks/results/2026-09-10/final-runtime/) identify the measured parser at `1262888`. Later evidence-only layers do not change that runtime. Every earlier report retains its own source identity; historical results are not silently assigned to newer code.

## Interpreting the gates

Early layers retain CI failures discovered during implementation. Later layers contain their fixes, including selected-allocation behavior on macOS and PBS validator/glibc integration. Historical failure logs and negative experiments remain part of the review history. A passing final head does not mean every intermediate layer passed independently.

Compatibility counts preserve failing assertions. Diagnostic experiments that raise an allocation retry ceiling explain behavior separately; they do not turn the original suite green. Likewise, normalized text callbacks, exact fragmentation, error positions and acceptance are separate contracts. Review the [C interface boundaries](../crates/oriole_expat/) before choosing a consumer.

The full distribution gate is scoped to an opt-in CPython 3.12.13 Linux x86-64 build. Other PBS targets require separate packaging and runtime evidence. The default PBS dependency remains Expat. The [acceptance criteria](../CONTRIBUTING.md#acceptance) remain open for a general production replacement.

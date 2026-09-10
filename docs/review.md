# Reviewing Oriole

The implementation is published in native GitHub Stack 3, starting at [PR #1](https://github.com/astral-sh/oriole/pull/1). Each layer uses the preceding layer as its base, keeping implementation changes and their evidence reviewable together.

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
| Subsequent compatibility and performance | [Constructor allocation](https://github.com/astral-sh/oriole/pull/86), [entity limits](https://github.com/astral-sh/oriole/pull/89), [attribute scans](https://github.com/astral-sh/oriole/pull/91), [event output](https://github.com/astral-sh/oriole/pull/92), [ATTLIST publication](https://github.com/astral-sh/oriole/pull/93) | Original API outcomes, ownership across callbacks, work limits and measured project performance |
| Documentation, allocation and build follow-up | [Toucan README](https://github.com/astral-sh/oriole/pull/94), [buffer reservations](https://github.com/astral-sh/oriole/pull/95), [profile-guided builds](https://github.com/astral-sh/oriole/pull/96), [version consistency](https://github.com/astral-sh/oriole/pull/97) | Exact license structure, allocation costs, held-out training, API identity and independent normal/optimized consumer validation |
| Version-consistent runtime evidence | [Real consumer benchmarks](https://github.com/astral-sh/oriole/pull/98) and [sustained fuzzing](validation/2026-09-10/version-consistent-fuzz/) | Paired normal/optimized comparisons, retained failed experiments, exact source and corpus identities, and completed sanitizer campaigns |

The earlier [full runtime report](validation/2026-09-10/final-runtime/) and [benchmarks](../benchmarks/results/2026-09-10/final-runtime/) identify the parser at `1262888`. Subsequent implementation layers change that runtime. The [current version-consistent report](validation/2026-09-10/version-consistent-runtime/) identifies source `4b11ace`, its normal/optimized shared/static libraries, all 417 original API failures, native allocation checks, CPython tests and W3C acceptance. The [buffer report](validation/2026-09-10/buffer-reservations/) retains the preceding source's native measurements. Every report retains its own source identity; historical fuzzing and distribution results do not certify newer code.

The [current sanitizer report](validation/2026-09-10/version-consistent-fuzz/) records six completed ASan campaigns on `4b11ace`: 14,502,453 executions, 26,689 initial seed files and 15,876 evolved corpus files. Every corpus byte, origin transformation, build identity and raw log is retained. The original incomplete review and later complete-results review are both preserved; no crash or timeout was observed in these bounded campaigns.

## Interpreting the gates

Early layers retain CI failures discovered during implementation. Later layers contain their fixes, including selected-allocation behavior on macOS and PBS validator/glibc integration. Historical failure logs and negative experiments remain part of the review history. A passing final head does not mean every intermediate layer passed independently.

Compatibility counts preserve failing assertions. Diagnostic experiments that raise an allocation retry ceiling explain behavior separately; they do not turn the original suite green. Likewise, normalized text callbacks, exact fragmentation, error positions and acceptance are separate contracts. Review the [C interface boundaries](../crates/oriole_expat/) before choosing a consumer.

The full distribution gate is scoped to an opt-in CPython 3.12.13 Linux x86-64 build. Other PBS targets require separate packaging and runtime evidence. The default PBS dependency remains Expat. The [acceptance criteria](../CONTRIBUTING.md#acceptance) remain open for a general production replacement.

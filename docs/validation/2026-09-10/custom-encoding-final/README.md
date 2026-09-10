# Custom encoding: final composition

This checkpoint validates `d89f839` on `ed7748a`, composing lexical provenance
with the reviewed token ownership, character-data coalescing, and input-context
copy changes. No runtime changes were needed after this rebase. The earlier
[original](../custom-encoding-provenance/),
[lifecycle](../custom-encoding-integrated/), and
[profiled](../custom-encoding-profiled/) checkpoints remain unchanged.

## Compatibility and allocation checks

- All 267 affected Rust checks, strict Clippy, and formatting pass.
- All 3,918 ordinary differential cases agree exactly with the preceding
  `ed7748a` library: status, errors, callback stream, and final position.
- The complete 4,740-configuration upstream API matrix improves from
  **3,999 passed / 741 failed** to **4,047 passed / 693 failed**. Exactly 48
  configurations improve; every other result and process disposition is
  unchanged. Both runs use identical adapted upstream sources and assertions.
- All 114,714 custom-converter grid cases retain their earlier results. There
  are no acceptance differences. Successful semantic callbacks agree in the
  grids without external-DTD default handlers; the separate default-handler
  grid retains 5,112 successful callback-prefix differences. Error codes,
  malformed-input callback prefixes, and positions can still differ.
- Six native C suites pass with shared and static linkage, including **333
  allocation-failure scenarios per linkage**. C is instrumented with ASan and
  UBSan; release Rust is uninstrumented and leak sanitizer is disabled.
- Independent source review found no blocker in the coalescing, alias
  restoration, or token-ownership composition. Its scope and file hashes are
  retained; it does not claim independent test execution.

The original four allocation-schedule failures introduced by token ownership
remain failures in both runs. Their assertions require a reallocation even at
failure index zero; increasing a retry ceiling does not address them. The
[prior classification](../custom-encoding-profiled/) preserves those assertions
and does not waive them. This checkpoint makes no zero-failure compatibility
claim. Stored entity-replacement CRLF normalization in attributes and NDATA
attribute error classification are separate preexisting gaps.

## Performance screen

Candidate/control throughput ratios compare this composition with `ed7748a`.
These are medians of five randomized pairs, with seven measured parses and a
warmup per process, pinned to CPU 1. Each pair checks callback hashes, element
counts, and text-byte counts. The inputs are pinned real-project XML, parsed
by the native callback driver; these are not timings of the project processes.

| Project input | 4 KiB feeds | 64 KiB feeds |
| --- | ---: | ---: |
| Vulkan | 0.944 | 0.934 |
| Wayland | 0.962 | 0.955 |
| Maven | 0.988 | 0.898 |
| Batik | 0.935 | 0.949 |
| GTK | 0.929 | 0.970 |
| DocBook | 0.960 | 0.918 |

Provenance retains a measured cost of approximately 1–10% in this screen.
Shared-host variation is substantial, particularly for Maven; all individual
samples and adverse generated fixtures are retained. This comparison does not
establish performance against Expat. The prior profiled checkpoint records the
ordinary-path optimizations and the rejected attribute-helper experiment.

## Artifacts and integration

The frozen shared library SHA-256 is
`beeab4e40f8cf11164f73ac5d50cfa169a3da5c8bfd0e984a10bca0f2912bd5a`;
the static archive is
`d181c033545c29c060774f6fd50639a918c2ab49c5c2e82028ad1f0cb392ec6f`.
`evidence.tar.gz` contains sources and hashes, the patch, commands, complete API
results, differential traces, converter probes, native logs, screening samples,
and the independent review. `files.json` lists archive members and checksums.

Later name-rule integration must validate raw lexical names and update
`custom_public_byte`; it must preserve the removal of decoded namespace-name
checks. Coalescing must continue scanning lexical representatives, with alias
restoration inside `character_data` while source metadata remains live.
`Buffer::swap_decoded` publishes raw callback text only after all lexical
projections finish. Iterative attribute expansion must retain the distinction
between physical whitespace and aliases, as well as the default-literal prepass.

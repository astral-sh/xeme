# Missing parameter references between declarations

Commit `3e4d006` fixes undeclared parameter references without changing name
validation or resource limits. Expat requires a declaration for a standalone
document reference outside an internal replacement; other references can be
skipped and suppress subsequent declarations. Disabled parameter processing
now delivers its raw default callback after NotStandalone. Custom encoded
names decode from their lexical slice before that raw callback is published.

## Validation

- 279 Rust checks and strict Clippy pass. Selected-allocator exhaustive OOM
  includes both enabled and disabled missing-reference handling.
- All six native C suites pass, including 333 allocation-failure scenarios per
  linkage. C uses ASan/UBSan; release Rust is uninstrumented and leak sanitizer
  is disabled.
- The complete 4,740-configuration API matrix remains exactly **4,065 passed /
  675 failed**, including every process disposition. No assertions are waived.
- All 1,944 focused oracle conditions have matching top-level acceptance.
  The original 24 child-execution and 162 callback differences remain unchanged;
  these involve external-subset NotStandalone timing and rejected-input prefixes.
- An 18-case custom-encoding oracle agrees exactly when both doctype callbacks
  are registered. It verifies decoded disabled-reference defaults across
  standalone modes and feed widths. The preceding library differed in all 18.

An initial probe without an end-doctype handler retains a preexisting extra `]`
in Oriole's default callback. Its original script and all 18 differing traces
remain in the archive; the narrower callback profile does not waive those
differences. Precreated and nonfinal sibling declaration-skip state is a
separate following layer.

## Evidence

The frozen shared library SHA-256 is
`75beb3faf3b5677567b91f9abce4122ce563bbc360f9484bce5daad693e06838`.
`evidence.tar.gz` contains sources and hashes, the original patch and integration
reject, complete API results, focused oracles, native logs, and commands.
`files.json` lists every archive member and checksum. No performance claim is
made for this semantic fix.

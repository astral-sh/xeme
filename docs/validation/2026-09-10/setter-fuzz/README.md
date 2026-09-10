# Encoding-setter fuzz coverage

The value-family target now changes protocol encoding metadata after a converter abort, after the root driver returns, and after a retained child finishes or aborts. It checks state rejection, allocation failure recovery, callback registration lifetime, release counts, and unchanged error/position state. The 29 additional seeds retain the existing limits on children, depth, requests, feeds, and resumes.

An isolated AddressSanitizer replay passed all 89 seeds (60 existing, 29 new), with no crash artifact and unchanged source and binary hashes. This replay uses the foreign-DTD plus API-state source snapshot described in `replay/source.json`; it predates the subsequent combined DTD build. It is a seed replay, not a timed final campaign. LeakSanitizer is disabled; selected-allocator live-block and release assertions remain enabled.

The handoff includes the patch, seed generator and manifest, and scoped Clippy log. The replay includes exact commands, inputs, source hashes and sanitizer output. The first build invocation lacked cargo-fuzz on PATH; its failure and the successful corrected build are both retained. Root reviewed pointer lifetimes and failure scheduling, and added only a SAFETY comment to the tested target.

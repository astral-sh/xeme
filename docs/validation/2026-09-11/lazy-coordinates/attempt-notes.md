# Lazy coordinate prototype local checks

- check-attempt01: `cargo +ohm -Zohm-defaults=no check --offline -p oriole -p oriole_expat --all-targets --jobs 1`, CPU4, exited101. The first compile found the remaining eager DTD `last_position` assignment required wrapping in `AdapterLocation::Position`. Raw stdout/stderr retained. The analogous private C width-test assignments were updated before the next compile. No parser/test ran in this attempt.
- New checks use run.py with write-once labels and exact command/environment/exit/stream hashes. Root benchmark hold remains in force before the next target run.

- Before first test run, independent review caught a fixture calling set_limits after feed (unsupported). Set the limit in initial Config instead, and leave A unresolved until after the rejected feed.

- After full443-test pass, self-review found the newly added short nonempty Start could allocate nothing after warming. Strengthened only that fixture: unresolved native Text lead precedes a longer element name that requires name-owner growth. The sweep now asserts it actually observed a failed target Start and resolved the preserved native A. Runtime source did not change. Rerun only affected allocator test plus formatting/Clippy.

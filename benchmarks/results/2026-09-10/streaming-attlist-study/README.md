# Ordinary ATTLIST continuation studies

We publish attributes from a complete declaration one at a time, with owned continuation state and DTD tables committed before callbacks. Earlier callbacks and callback-created parameter children can affect later attributes. Incremental input still waits for the complete lexical token.

The original study reduces peak selected allocation for 10,000 CDATA declarations from 15.1 MB to 9.5 MB with declaration handlers, and from 21.8 MB to 9.5 MB with only Default callbacks. Enumeration callbacks add one capture allocation per attribute. Ordinary-project timing was mixed and slightly slower; this is a compatibility and bounded-memory change, not a general speedup. Original allocation sweeps, timings, semantic comparisons, source snapshots and negative intermediate candidates remain in the archive.

Root review found that an intermediate branch skipped repeated-element-name work charging when enumeration callbacks were absent. The repaired source retains the original 8 MiB boundary: 1,025 attributes with an 8,192-byte element name succeed, while 1,026 reject with error 43. The earlier bypassing candidate and its failing gate remain preserved. Omitting unused semantic callback payloads deliberately omits their adapter payload-byte charge; the configured counters and limits are unchanged, but complete counter parity is not claimed.

A broader custom-encoding comparison initially stopped at 1,356 changed observations. Independent comparison against actual Expat proves all of them are error byte-index corrections, with unchanged acceptance, error codes and callback content/order. The managed-Python rerun and all original observations are retained. Other callback, position and resource-policy differences remain explicit.

The original study and first composition use earlier source layers; their results are not assigned to the current integrated binary. See the current integration report for source and binary identity, full API results and new measurements.

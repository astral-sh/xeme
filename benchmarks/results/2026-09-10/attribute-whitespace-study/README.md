# Attribute whitespace batching study

This original isolated study replaces per-character attribute copying with contiguous runs between XML whitespace markers. Converted ASCII aliases retain their scalar provenance-aware path. It measured an 11.0% geometric speedup across 24 project conditions, with all medians improving; Batik XML_Parse instruction counts fell by 43.2%.

The author baseline predates shared DTD publication and the deeper C entity policy. Its 127 focused Rust checks, exact differential and custom-encoding probes, six native C sanitizer runs and unchanged 4,077/663 upstream API result belong to that source. The manifest records every command, source hash, raw timing sample and limitation. C sanitizers do not instrument the Rust library, and leak sanitizer is disabled.

The final combined source and current-base benchmark result are in [attribute-scans-composed](../../../../docs/validation/2026-09-10/attribute-scans-composed/). Do not multiply the original isolated gain into that result. Binary files and Python bytecode are excluded from the archive; their recorded hashes remain in the original manifest.

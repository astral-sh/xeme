# Caller-owned event output study

The parser writes an owned event into the adapter's local storage and returns its small recycling token separately. The event and its callback data remain owned after the core borrow and DTD publication guard end. This avoids an intermediate event copy without changing the public owned-event interface.

The original isolated screen improves all 24 project conditions, with a 1.058× geometric speedup. The generated declaration fixture at 64 KiB regresses by about 3.3%; raw samples and shared-host limitations remain explicit. Focused Rust checks, full upstream observations, encoding/streaming probes, native C sanitizers and independent ownership review are recorded in the author README and manifest inside the archive. The final patch adds tests and corrects a SAFETY comment's placement; all final source hashes are recorded.

Two later attempts to remove the remaining adapter dispatch copy were rejected at the code-generation/instruction-profile stage. Their source, assembly and profiles are retained separately. Neither received a throughput or broad correctness campaign. The final integrated parser is validated and benchmarked separately from this author study.

# Conditional DTD validation

This layer adds nested INCLUDE/IGNORE sections in external DTDs, including internal
parameter entities that select the keyword. The ten focused Rust tests cover
UTF-8/UTF-16 chunk boundaries, discarded markup, malformed input, entity boundaries,
disabled and missing references, default callbacks, skipped declarations, and limits.
The custom-allocator failure sweep includes both expanded and skipped headers.
Clippy passes; the four upstream Expat ignore-section tests pass in all twelve
chunk/deferral contexts (48 checks).

Independent review found and corrected split keywords across parameter entities,
missing active-source depth in the header limit, disabled/missing-reference behavior,
value expansion in skipped declarations, and callback-record amplification.
Conditional nesting uses the depth ceiling, whole ignored sections use the token
ceiling, and queued skipped-reference records consume the shared indirect-byte budget.

The final reference probes retain all outcomes. All 54 valid-child baseline cases,
all 72 skipped-value acceptance cases, and nine isolated child callback-rejection
cases match Expat. Two skipped-value diagnostic codes still differ. Malformed headers
retain earlier Default/NotStandalone callback differences, and parent NotStandalone
timing differs in six rejection cases. The ordinary-declaration probes retain the
unsupported parameter-reference-in-value cases. These differences are not waived.

The compressed reports include exact inputs, reference/candidate hashes, commands,
and the independently written probes. `source.json` and `source.tar.gz` identify the
isolated layer; `index.json` hashes the reports and release library. Build with
`cargo +ohm build --release --locked -p oriole_expat` and run the upstream harness
with the four test names recorded in its manifest. This evidence predates the
separate multibyte conversion layer and does not establish complete DTD compatibility.

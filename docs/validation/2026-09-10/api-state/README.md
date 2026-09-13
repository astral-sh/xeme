# Completed-parser API state

`XML_ParseBuffer(parser, 0, final)` can finish previously supplied input once
parsing has started, including reporting an incomplete character instead of
`XML_ERROR_NO_BUFFER`. Initialized parsers and positive lengths still require a
successful buffer reservation. Reentrant, suspended, failed, and finished parsing
retain their existing guards.

`XML_SetEncoding` now accepts completed or irreversibly aborted parsers, including
an event/converter callback that just aborted. It copies only protocol metadata
with the selected allocator, preserving custom maps, pending conversion, positions,
release callbacks, and terminal parser state. Allocation failure is transactional.

The focused upstream matrix passes all 108 contexts, adding 24 with no regression.
Independent probes cover 98 API states, 30 custom-map lifecycles, and 27 converter
abort cases. The remaining differences are explicitly recorded baseline behavior;
all 155 integrated observations equal the frozen isolated candidate. Full workspace
checks, strict Clippy, selected-allocation failure sweeps, and callback/reset/free
regressions pass, including exactly-once release after an aborted conversion.

The isolated evidence also retains an initial failed test fixture that neglected
to reinstall callback user data after reset. The fixture was corrected; no runtime
change was needed for that failure. The patch, raw observations, test output,
reviews, and separate isolated/integrated source and library identities are retained.

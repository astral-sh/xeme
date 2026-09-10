# Declared UTF-16 mismatch diagnostics

UTF-8 bytes declaring generic `UTF-16` now report `XML_ERROR_INCORRECT_ENCODING`, matching the existing endian-qualified rejection. Known UTF-16 names never invoke the custom-encoding handler. The error retains the encoding value's original byte, line and column location, including a UTF-8 BOM or declaration line breaks. Explicit protocol encodings retain precedence; correctly encoded UTF-16 retains byte-order detection.

The final library SHA256 is `63ac372c1692786eab161a66a5d39405870c533e7451e35b4fec0296e9f0c86b`. It includes the PR56 runtime plus this change; PR57 changes only benchmark tools and evidence. The source manifest, patches, exact oracle inputs, original/final observations and build commands are retained in the archive.

All 60 focused upstream Expat test configurations pass, including 12 previously failing `test_not_utf16` configurations. The core's 12 encoding tests and strict Clippy pass. The regression covers all three parser contexts, generic/mixed-case/endian-qualified names, BOMs, and chunk boundaries.

The independent C oracle records 768 observations across document, DTD and content children, explicit protocols, valid UTF-16, unknown names, declaration line breaks, and malformed declaration controls. Compared with the preceding runtime, 120 status/error/custom-hook observations and 96 byte positions improve, with no new outcome, callback or position differences. Complete callback comparison still has 480 differences: 288 declaration-callback-only differences, 96 malformed-input error differences, 48 existing explicit-protocol/BOM outcome differences, and 48 malformed error/custom-hook differences. Expat can emit the XML-declaration callback before rejecting an encoding mismatch; Oriole still rejects before that callback. These failures are not waived.

The initial candidate corrected the error but moved generic UTF-16 diagnostics to byte zero. The final revision reuses declaration-value location tracking for unknown and incorrect encodings. Both runs are preserved. These focused checks do not replace the full combined API matrix, distribution validation, or sanitizer campaigns.

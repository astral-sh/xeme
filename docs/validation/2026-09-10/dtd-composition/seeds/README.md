# DTD composition family seeds

The 54 new inputs use the unchanged ffi_family target. They cover UTF-8 and both UTF-16 byte orders, delimiter tails, nested references, malformed lexical boundaries, surviving children after parent reset/free, and six selected-allocation failure points.

The 48 semantic cases match reference Expat and the isolated DTD implementation. Oriole additionally supports retaining children after parent destruction/reset; reference Expat keeps its parent alive during its oracle run. The initial generator mistakenly exercised that lifetime extension on Expat and crashed. Its source is retained separately, and the corrected generator records the reference/candidate lifetime distinction. These files record seed construction and reference observations; sanitizer replay is a separate gate.

Root checked every seed hash and verified that the controls select the intended DTD child, parameter mode, parent lifetime and two-byte feed operations. No harness code or execution bounds changed.

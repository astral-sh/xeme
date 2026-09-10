# Internal parameter entities in declaration tokens

Internal parameter entities can supply complete lexical tokens inside external DTD
declarations. Borrowed frames keep names, quoted literals, and references inside
their original entity boundaries while allowing content-model grammar to continue
across frames. Literal provenance preserves newline normalization, recursion checks,
and combined declaration, value, and inherited child-depth limits. Missing parameter
references preserve earlier attribute callbacks and suppress later declarations when
required. Metadata, output, and empty-reference work are bounded before allocation.

The isolated layer passes 150 core/C-interface tests and strict Clippy. The combined
header/declaration source passes 162 tests, formatting, and strict Clippy. Independent
merge review confirms that inherited ancestry and depth accounting remain intact.
All 9,536 UTF-8/UTF-16 chunk summaries and 42 literal-provenance cases match.

The focused 318-case matrix removes 186 baseline acceptance discrepancies and leaves
none; 12 malformed error-code differences remain. A broader 1,680-case matrix has no
acceptance differences and retains 92 error-code differences. Successful declaration
payloads and concatenated Default bytes match in both matrices with EntityDecl and
Attlist handlers registered. Six separate Default-only reproductions retain missing
or duplicate raw fragments; exact handler-dependent callback timing is incomplete.
External parameter references within declaration grammar or values remain unsupported
at this layer. Whole declaration delimiters cannot span parameter entities.

The evidence archive contains the isolated patch and source manifest, the integrated
source snapshot and test logs, complete oracle observations and generators, and the
independent merge review. Frozen source identities distinguish the isolated oracle
library from the combined implementation in this PR.

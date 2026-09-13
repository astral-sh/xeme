# XML minor-version declarations

The [XML 1.0 Fifth Edition grammar](https://www.w3.org/TR/2008/REC-xml-20081126/#sec-prolog-dtd)
allows `1.` followed by one or more ASCII digits. We accept those declarations
and retain the reported version while applying XML 1.0 character rules. This
fixes the pinned W3C corpus case `x-rmt-008b`, whose version is `1.7`.

The 360-case oracle spans root documents, external content, external DTDs, and
external entity values with whole, one-byte, and three-byte feeds. The change
fixes 60 outcome differences and adds none. The remaining 174 observations are
retained: Expat accepts some version strings outside the grammar, and existing
external text-declaration validation/error-code differences remain. The separate
704-case namespace oracle retains its prior 32 differences.

Focused Rust regressions cover callback version values, forward-compatible
minor versions in external replacement values, malformed version strings, and
rejection of XML 1.1-only character references. This change does not implement
the XML 1.1 character grammar. Compressed raw results contain both libraries'
SHA-256 identities, callbacks, child outcomes, and chunk sizes.

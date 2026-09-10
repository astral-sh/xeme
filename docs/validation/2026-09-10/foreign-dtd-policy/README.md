# Foreign DTD policy and callback ordering

We distinguish an absent handler, a handler that declines a foreign DTD, a child
that is only created, and a child that receives input. A read non-standalone DTD
triggers the caller's NotStandalone policy before end-doctype or root callbacks.
Declining restores the previous possible-subset state, so unknown entities remain
errors when no earlier parameter reference made the subset incomplete.

The isolated 4,632-case oracle has zero outcome differences and fixes 1,866
previously different complete callback observations without a regression. Its
420 remaining differences are existing malformed-child diagnostics (240) and
disabled internal-parameter Default omissions (180). A second 480-case oracle
matches Expat exactly for nested missing, declined, created-only, empty, and
parsed entities, including policy rejection and declarations around references.

The focused upstream matrix passes 96 of 120 contexts, adding 24 passes. Its
24 failures remain visible: allocation retry ceilings and exact invalid-child
diagnostics. Whole-workspace tests and strict Clippy pass after integration over
the namespace and version fixes. Repeating all 4,632 cases on that combined
release preserves the same outcomes and 420 retained differences.

The archive contains the isolated patch, source/library identities, independent
oracles, reviews, and raw results. The root review and compressed logs identify
the integrated source and release separately. No final-head performance claim
is inferred from the earlier measured runtime.

# Combined namespace, version, foreign-DTD, and encoding API checkpoint

The release at `e84865d` passes **3,705** of 4,740 adapted public Expat 2.8.4
contexts and fails **1,035**. The identical public suite/reference contract is
documented in the [combined evidence](../combined-evidence/). All original
assertions remain unchanged; failures are not waived.

Compared with `b68bdca`, 84 contexts improve and none regress: 24 foreign-DTD
policy contexts and 60 external-content encoding contexts. All remaining failures
exit through assertions (100), with no signals or timeouts. The matched reference
passes all 4,740 contexts at the same 4 GiB/15-second bounds.

The archive retains the adapter, full observations, comparison, compile/test logs,
and library/source identities. It extends the older full matrix; the older
failure classification describes its own frozen runtime and is not silently
rewritten to count these fixes.

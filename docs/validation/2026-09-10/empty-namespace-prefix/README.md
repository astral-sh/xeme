# Reject an empty declared namespace prefix

The W3C Namespaces 1.0 case `rmt-ns10-016` exposed that `xmlns:` was treated as bare `xmlns`. We now require the portion after `xmlns:` to be a nonempty namespace name. Bare `xmlns` still declares or clears the default namespace, and namespace-disabled parsing retains ordinary XML name rules.

Namespace regression tests pass. Independent review compares 704 explicit/defaulted declaration, URI, namespace-mode, and chunk cases: 16 prior outcome differences are fixed and no new differences appear. The remaining 32 are earlier reserved-prefix empty-URI precedence and defaulted invalid-prefix differences. The complete observations, baseline comparison, generator, tested library hashes, and review are retained.

The shared generator also probes version declarations; those observations informed a separate change and are not claims made by this namespace fix.

# Rejected parser optimizations

Two source experiments failed to provide a useful improvement over the selected Context runtime. Neither changes the shipped parser.

| Experiment | Normal real-project time vs control | PGO real-project time vs control | PGO generated time vs control |
|---|---:|---:|---:|
| [Default-namespace lookup shortcut](default-namespace-guard/README.md) | +0.69% | +0.39% | −1.38% |
| [CDATA terminator prefilter](cdata-prefilter/README.md) | +0.89% | −0.07% | +2.24% |

Positive values mean slower. These are geometric means of paired condition medians. Each study retains every condition, including regressions, and separate comparisons with Expat. The CDATA PGO result is effectively flat.

Both experiments passed local tests, formatting, Clippy, matched normal/PGO builds and callback checks. Independent readers verified source identities, compiler arguments, training records and all 1,344 native worker records per experiment. Each package can replay its saved results after relocation without loading a parser or compiler.

These experiments do not close the performance gap or establish production readiness. The selected runtime and its existing validation limits remain as documented in [the Context study](../context-text-frame/README.md). The target remains roughly within 10% of Expat on held-out project XML through both C and CPython.

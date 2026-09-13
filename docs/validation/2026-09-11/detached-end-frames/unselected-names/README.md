# Unselected name optimizations

The expanded namespace frame chain and ASCII NameChar table remain experiments. None is included in the closing-tag runtime. All original preflights, measurements, regressions, source/build identities and independent reviews are preserved in [evidence.tar.gz](evidence.tar.gz), indexed by [members.json](members.json) and [manifest.json](manifest.json).

| Candidate | Result and decision |
| --- | --- |
| B1 expanded frames | Reject: equal raw/expanded names increased malloc/realloc calls from 0/1 to 129/129 in the NUL-separator case. |
| B2 equal-name owner | Ownership fix retained; native ratio 1.006380 with 20/24 regressions. No Python or full gates. |
| B3 outlined expansion | Targeted attribute inlining criterion failed. No profiles or elapsed study. |
| B4 ordinary inline hint | Generated `.text` is byte-identical to B3. No profiles or elapsed study. |
| B5 forced tiny append inlining | Native ratio 0.985190, 15/24 regressions; CPython 0.982512, 12/24 regressions. Namespace-heavy gains with common/generated costs; held. |
| ASCII NameChar table | Instructions improve 2.58%; native ratio 0.992047 with 6/24 regressions, CPython 0.997092 with 9/24 regressions. Both native entity conditions slower; held. |

B2/B5 use the earlier `a55` control; ASCII uses Cell `0cfd`. They were not composed with End or each other. B5's initial wrong-affinity preparation stopped before parser execution; the failure and corrected preparation remain. ASCII's first worktree destination refusal is also retained. No full API, strict 802-test CPython, PBS or sustained sanitizer claim applies to these candidates.

The outer archive has 6,002 direct members, 22 binary exclusions and SHA-256 `88572916fc7e7ae914ae8408c9612290f8aa74269ea09005d1833cc1c2b0be28`. Its packager rechecked every direct member and origin. Nested archive bytes were verified; their individual internal members retain the original scoped review receipts rather than a new complete outer audit.

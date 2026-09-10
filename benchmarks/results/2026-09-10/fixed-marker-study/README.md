# Fixed delimiter search study

This archive preserves the original isolated `]]>` search study, including its first lint failure, corrected source, exact validation, instruction profiles, raw native timing samples and generated controls. The author README inside the archive describes the source phases and validation scope.

The final isolated library retains all 4,740 upstream observations on its older baseline, at 4,077 passing and 663 failing configurations. The fixed byte search reduces XML_Parse instructions on Wayland and Maven. Its timing gains and regressions remain separate from the later current-base result in [attribute-scans-composed](../../../../docs/validation/2026-09-10/attribute-scans-composed/).

The current-base attribution accepts this change with a 1.038× geometric project speedup and 23 of 24 conditions improving; the generated declaration fixture is about 4% slower. Source and artifact manifests identify both studies. No blanket speedup or new full sanitizer campaign is claimed for the original isolated study.

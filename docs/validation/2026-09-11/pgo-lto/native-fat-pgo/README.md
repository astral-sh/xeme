# Current PGO: fat versus thin native evidence

Fat-PGO took 6.76% longer than ThinPGO across the 24 real native conditions (geometric mean); all 24 regressed. The four generated controls averaged 4.64% longer, with both entity cases regressing and both rare-declaration cases improving.

The fixed study retained 84 preflights, 588 timed workers, 196 seeded cohorts, and 105,420 measured samples. CPU4 Python preparation overlapped the CPU0 lifecycle timing. No reruns or tuning.

`evidence.tar.gz` contains complete raw records, controllers, all condition results, the independent native audit, and the independent fresh fat-PGO build/training audit. `readback.json` records verification of every archive member. Detailed inventory is compressed inside the archive. Source/build bytes are preserved in the earlier `oriole-current-pgo-native-handoff` archive identified by `report.json`. Actual Python and fresh ThinPGO correctness gates are separate.

# Current ThinPGO C correctness evidence

The measured ThinPGO library preserves all 4,740 original API outcomes: 4,347 pass and 393 fail, with original API exit 1 and bounds of 3 seconds/test, 1 GiB address space, 768 MiB RSS, and 240 seconds total.

Both linkages pass the original C integration, adversarial, and 327-scenario allocation suites. Saved parity checks cover 3,318 strict traces, 36,456 custom-alias comparisons, 2,392 malformed comparisons, 1,304 publication cases (3,912 parses), and the original End38/End6 fixtures. Existing reference callback-position differences remain.

The baseline is the previously verified PR116 implicit-target library ccfb/f58; the measured candidate is current ThinPGO 4f/8b6. All70 source bytes match. The fresh explicit-target performance control 0c8c is a separate artifact.

`evidence.tar.gz` preserves source, scripts, commands, bounds, raw outputs, summary, and detailed inventory. Executable binaries are excluded with exact hashes retained. All outer and 70 nested source member hashes were read back. Independent C-gate audit is a separate receipt.

Limits: C sanitizer instrumentation excludes the Rust release library; leak checking is disabled. The unchanged custom-alias helper does not retain every successful raw pair. Root owns strict CPython evidence separately. No fresh workspace claim. These CPU1 gates overlapped allocator timing on CPU0 and strict CPython on CPU4. No new failed attempts or reruns.

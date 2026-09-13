# Independent native byte-count C-gate audit

The saved-data audit passed on its first attempt: **15,849 checks and 375 inspected files**. No parser, target, build, compiler or timing was executed. Review work ran on CPU 4; saved gate commands ran on CPU 1.

## Verified evidence

- All **4,740** API rows were reconstructed from the raw test log and match the baseline byte-for-byte: **4,347 pass, 393 fail, exit 1**. Original test selection and resource limits remain.
- Six C ASan/UBSan consumers completed for dynamic and static linkage, with 327 allocation scenarios per linkage. Rust release libraries remain uninstrumented; leak checking was disabled.
- All **3,318** strict trace pairs and **2,392** malformed pairs match in every saved field. The malformed input hashes, encodings and feed sizes were independently reconstructed.
- All **1,304** publication cases (**3,912** parses) match the baseline. Nine same-process symbol origins are verified. Reference differences remain limited to 10,416 default-callback and 864 external-callback position events.
- End lifecycle results retain all 38 cases for each of three engines, 21 symbol origins per engine, and the historical reference differences. End allocation results retain 37 rows per engine, six scenarios, four selected failures, two success controls, and scenario 4's retry error 33.
- Custom alias evidence preserves **36,456** comparisons with source-derived loop counts, hashes, exits and empty difference summaries. The inherited helper does not retain successful raw pairs; the audit does not claim to reread those pairs.

## Failure and source history

The initial CPU 4 preparation is separately retained. All 13 inherited controllers and the subsequent CPU 1 edits were checked. The initial End controller failed its remaining CPU 4 guard before launching a parser (`runs=[]`). Its correction changes only CPU guards and metadata; the separate remaining-work controller ran End38 and End6 afterward. Successful earlier probes remain intact.

The initial final-summary attempt failed because saved-data classifications were not yet present. Its log and script remain. The classifier produced the missing readbacks, then the identical final summarizer completed. No parser or compiler rerun was needed for that postprocessing.

Source `735afb94`, candidate `ccfb2295` / `f58d5bc6`, and selected control `02fcab59` / `69ebed41` are pinned. The separately sealed source/build review links the integrated source to the measured library bytes. Workspace and CPython gates are outside this C-gate audit.

`prepare-audit.py` retains the prior complete End+Cell raw audit and End supplement with an auditable adaptation. `generator.py`, `audit-setup.py`, `audit-tail.py`, `checks.json`, `checked-files.json` and `attempt-01.*` preserve the review. `receipt.json` and `files.json` provide the final readback seal.

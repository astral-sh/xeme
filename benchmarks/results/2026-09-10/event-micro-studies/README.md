# Excluded scanner and accounting candidates

These three isolated candidates each reduced selected instruction counts: avoiding a second zero-byte source charge, skipping an 88-byte empty parameter-record move, and using a two-byte attribute marker search. The first had an older-base 1.020× geometric project speedup; the marker change reduced Batik instructions by 11% but its project timing average was near neutral. The empty-record candidate had no isolated timing campaign.

Their source, exact build identities, original validation and raw measurements are retained here. Validation scope differs by candidate: the empty-record study has 323 complete core/adapter Rust checks, while the other two were release-built and exercised by compatibility/native probes without a full isolated Rust suite. The full combined source later passed 326 Rust checks and the broader gates in [event-output-composition](../event-output-composition/).

Current-base attribution did not establish a useful aggregate benefit beyond event output alone, and generated declarations regressed about 5%. All three changes are excluded from the selected runtime. Instruction reductions are not presented as throughput gains, and older-base gains are not multiplied together.

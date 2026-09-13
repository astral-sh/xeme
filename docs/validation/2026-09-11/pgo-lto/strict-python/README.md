# ThinLTO PGO strict CPython validation

[Independent review](review.json) verifies exact outcome parity for both shared and static builds: 802 method outcomes each, two retained failures, 14 reported skips and three expected failures. This is not a green-suite result.

[Archive](evidence.tar.gz) retains both original test logs, source/commands/loader origins, baseline logs, upstream test modules and the complete independent reconstruction. [Members and origins](archive-members.json.gz), [excluded compiled binary hashes](excluded-binaries.json), and [readback receipt](receipt.json) define its exact scope.

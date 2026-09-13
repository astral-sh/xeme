# Reserved project XML holdout

These five projects were selected by production document role before any XML
parsing or timing. The complete upstream blobs are reserved for one final
evaluation after code selection. Do not use this corpus to select or tune a
candidate change. Once results have been observed, this corpus is no longer an
unseen holdout for subsequent changes.

| Project | Original input | Bytes | Role |
| --- | --- | ---: | --- |
| LibreOffice | `officecfg/registry/schema/org/openoffice/Office/Calc.xcs` | 81,280 | Namespace-qualified application configuration schema |
| .NET runtime | `src/libraries/System.Private.CoreLib/src/System.Private.CoreLib.Shared.projitems` | 254,307 | MSBuild source-item and build metadata |
| Apache Hadoop | `hadoop-common-project/hadoop-common/src/main/resources/core-default.xml` | 165,527 | Configuration with descriptions and comments |
| Qt translations | `translations/qtbase_de.ts` | 276,105 | German Unicode translation catalog |
| MuseScore | `share/instruments/instruments.xml` | 820,743 | Hierarchical instrument catalog |

Each project has exactly one `role: input` entry in
[`corpus-manifest.json`](corpus-manifest.json). Licenses and notices have separate
roles. Every listed file records a full upstream commit, original path, source
URL, Git blob SHA-1, byte count and SHA-256. The manifest is compatible with
`projects.py`'s existing `projects`/`files` input selection. No input has been
rewritten, concatenated, repeated, truncated or regenerated.

## Selection and provenance

The five project roles were approved before payload acquisition. Initial paths
are retained in [`selection-paths.json`](selection-paths.json); upstream branch
and commit selections are in
[`selection-repositories.json`](selection-repositories.json). No size or
performance threshold was used to substitute inputs.

The initial .NET path omitted `/src/` and returned HTTP 404. Directory metadata
located the same named CoreLib shared-project file. A Qt
`translations/LICENSE` lookup also returned 404; the repository license rule and
all four available `LICENSES` texts were copied instead. Both failures and the
exact successful `gh-auto` commands are preserved in
[`acquisition.json`](acquisition.json).

LibreOffice declares a fixed relative external DTD. The declaration is retained;
the benchmark must keep external resolution disabled. Lexical byte inspection
found no named references beyond XML's five predefined entities in any selected
input. That inspection is not a parser/conformance result. Qt's `DOCTYPE TS`
has no external identifier. Hadoop's stylesheet processing instruction is data,
not a request to execute a transform.

MuseScore's own header says upstream generated its committed production catalog
from its instrument spreadsheet. The acquired file is that original committed
output; this acquisition does not fetch the spreadsheet or run the generator.

The original license headers remain in each input. LibreOffice's MPL text,
Apache incorporation notice and complete upstream license document are copied.
.NET and Hadoop include their repository license and notices. Qt's catalog has
no per-file SPDX header: the copied repository `licenseRule.json` supplies its
default module rule, including GPL-3.0-only; the complete available upstream
license texts are retained. MuseScore's complete GPL-3.0 license file is copied.
The manifest records these bases without replacing upstream terms.

## Independence check and freeze

[`absence-check.json`](absence-check.json) records a check at Oriole commit
`fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f`: none of the five complete-file hashes
matches 266 tracked corpus/seed files or appears in 1,756 tracked JSON,
JSON.gz or Markdown evidence records. These projects differ from Vulkan,
Wayland, Maven, Batik, GTK and DocBook. This check does not establish absence of
shared fragments, related schemas, unrecorded local experiments, or inputs
inside unsearched archive formats.

[`freeze.json`](freeze.json) binds this acquisition's manifest and files before
any parser, compiler, preflight or timing execution on these inputs. Its status
reserves the corpus; it does not certify successful parsing, callback
equivalence, performance or XML conformance. The final evaluation must bind the
selected code and libraries separately and retain all failures and conditions.
Do not replace a file after seeing a parser or timing result.

## Reacquisition

From the repository root, with GitHub CLI authenticated:

```console
python3 benchmarks/holdout/fetch.py --output /tmp/oriole-holdout-reacquired
```

On the managed devbox, add `--gh /home/dev-user/.local/bin/gh-auto` to use its
scoped OSS account routing. Reacquisition uses read-only GitHub API requests.

`fetch.py` reissues the pinned GitHub contents requests, decodes base64 directly
to bytes, and checks all three recorded identities before writing each file.
It requires a new output directory and retains partial acquisitions on error.
It does not parse XML or run a benchmark. Exact original API commands and
identity-check details are also available in `acquisition.json`. Network access
and continued availability of the pinned upstream objects are required.

## Verification and final evaluation

`python3 benchmarks/hillclimb.py verify-holdout` checks every listed input and
license/notice file against its byte count, SHA-256 and upstream Git blob hash,
then checks the frozen manifest and its separation from the tuning projects.
It performs no parsing or timing.

After selecting and freezing the candidate using tuning results and correctness
review, use `hillclimb.py run --mode holdout --selection-note ...` as described in
[the iteration guide](../HILLCLIMB.md). The note records the decision made before
observing these results. The mode uses all five inputs (20 native and 20 Python
conditions when both consumers run), with separate holdout aggregates; it does
not replace or pool the six tuning projects. The same 16 generated native
controls remain separate. Complete canonical preflights run before measurement;
any failure is retained, not grounds for replacing a file.

The tool records inputs and the prior selection note but cannot enforce whether
a person has already viewed results. Do not repeatedly choose code changes from
this holdout's timings. After exposure, retain it as a regression corpus and
select a new unseen holdout for the next independent evaluation.

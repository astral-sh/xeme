# Project XML holdout

**Evaluated on 2026-09-13.** The [first evaluation](../../docs/evidence/2026-09-13-review.md#performance)
used runtime `ec4d068` and retained every input and condition. Both real
aggregates missed the 1.20× Expat goal. These inputs are now a regression corpus;
an independent evaluation needs a new, unseen corpus.

The five projects were selected by document role before parsing or timing, and
reserved until after candidate selection. The records below document their
acquisition and the freeze made before evaluation.

| Project | Original input | Bytes | Role |
| --- | --- | ---: | --- |
| LibreOffice | `officecfg/registry/schema/org/openoffice/Office/Calc.xcs` | 81,280 | Namespace-qualified application configuration schema |
| .NET runtime | `src/libraries/System.Private.CoreLib/src/System.Private.CoreLib.Shared.projitems` | 254,307 | MSBuild source-item and build metadata |
| Apache Hadoop | `hadoop-common-project/hadoop-common/src/main/resources/core-default.xml` | 165,527 | Configuration with descriptions and comments |
| Qt translations | `translations/qtbase_de.ts` | 276,105 | German Unicode translation catalog |
| MuseScore | `share/instruments/instruments.xml` | 820,743 | Hierarchical instrument catalog |

Each project has one `role: input` entry in
[`corpus-manifest.json`](corpus-manifest.json). Licenses and notices have separate
roles. Every listed file records a full upstream commit, original path, source
URL, Git blob SHA-1, byte count and SHA-256. The manifest is compatible with
`projects.py`'s `projects`/`files` input selection. Inputs are unmodified upstream
files.

## Selection and provenance

The five project roles were chosen before downloading the inputs. Initial paths
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

MuseScore's header says the catalog was generated from its instrument
spreadsheet. The corpus uses the committed catalog without running the generator.

The original license headers remain in each input. LibreOffice's MPL text,
Apache incorporation notice and complete upstream license document are copied.
.NET and Hadoop include their repository license and notices. Qt's catalog has
no per-file SPDX header: the copied repository `licenseRule.json` supplies its
default module rule, including GPL-3.0-only; the complete available upstream
license texts are retained. MuseScore's complete GPL-3.0 license file is copied.
The manifest records these license sources.

## Independence check and freeze

[`absence-check.json`](absence-check.json) records a check at Oriole commit
`fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f`: none of the five complete-file hashes
matches 266 tracked corpus/seed files or appears in 1,756 tracked JSON,
JSON.gz or Markdown evidence records. These projects differ from Vulkan,
Wayland, Maven, Batik, GTK and DocBook. This check does not establish absence of
shared fragments, related schemas, unrecorded local experiments, or inputs
inside unsearched archive formats.

[`freeze.json`](freeze.json) records the manifest and file hashes from before any
parser, compiler, preflight or timing execution on these inputs. It records
provenance; parsing, callback equivalence, performance and XML conformance need
separate checks. The evaluation records must also identify the selected code and
libraries and retain all failures and conditions.

## Reacquisition

From the repository root, with GitHub CLI authenticated:

```console
python3 benchmarks/holdout/fetch.py --output /tmp/oriole-holdout-reacquired
```

On the managed devbox, add `--gh /home/dev-user/.local/bin/gh-auto` to use its
scoped OSS account routing. Reacquisition uses read-only GitHub API requests.

`fetch.py` reissues the pinned GitHub contents requests, decodes base64 to bytes,
and checks each file's byte count, SHA-256 and Git blob hash before writing it.
It requires a new output directory and retains partial downloads on error.
It does not parse XML or run a benchmark. `acquisition.json` records the original
API commands and hash checks. Reacquisition needs network access and the pinned
upstream objects must still be available.

## Verification and final evaluation

`python3 benchmarks/hillclimb.py verify-holdout` checks every listed input and
license/notice file against its byte count, SHA-256 and upstream Git blob hash,
then checks the frozen manifest and its separation from the tuning projects.
It performs no parsing or timing.

Use `hillclimb.py run --mode holdout --selection-note ...` as described in
[the iteration guide](../HILLCLIMB.md). The selection note identifies the chosen
candidate and its rationale. The mode uses all five inputs: 20 native and 20
Python conditions when both consumers run. Holdout aggregates, tuning results
and the 16 generated native controls are reported separately. Complete canonical
preflights run before measurement. Retain failures and adverse conditions;
do not replace inputs based on their results.

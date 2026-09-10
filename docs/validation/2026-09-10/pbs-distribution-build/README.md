# First complete PBS build

[Run 34436738490](https://github.com/astral-sh/oriole/actions/runs/34436738490)
built CPython 3.12.13 with Oriole at the source-pin layer (`20bba5a`), using the
pinned PBS revision and the actual Linux x86-64 Docker build. The build completed
and produced a distribution archive; its exact name and SHA-256 are in `index.json`
and the retained complete build log. This source predates the subsequent conditional,
multibyte, diagnostic, and parameter-value changes.

Distribution validation did not run: Cargo treated the nested `.cache/pbs` package
as part of Oriole's workspace and refused to build the validator. The generated
interpreter's XML suites, installed parser identity check, and older-glibc symbol
validation consequently did not run. The overall CI run failed. A successful build
alone does not establish distribution portability or production readiness.

The follow-up excludes the nested PBS package from Oriole's workspace, builds the
validator before the long Python build, and retains the generated experimental
archive as a seven-day CI artifact before validation. Local Cargo metadata confirms
the actual pinned PBS package now forms its own workspace. The complete distribution
gate reruns against the newer combined source.

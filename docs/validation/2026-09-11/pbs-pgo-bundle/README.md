# Fresh PGO bundles for PBS

PBS can now select a freshly trained Oriole PGO archive through the existing bundle recipe. The bridge uses the generated-only PGO pipeline, verifies the source, compiler, configuration, profiles and actual PIC/unwind/ThinLTO compiler commands, and packages the exact profile-use static archive. Native linker dependencies come from that same compilation. The existing header, license, CPython cleanup-backport and weak TLS-hook checks remain shared with the normal recipe.

The **stable-toolchain PGO PBS distribution trial is pending**. The completed local build used Ohm with experimental defaults disabled. These records establish the integration path; they do not add a new elapsed benchmark or change the selected parser runtime's compatibility results.

| Evidence | Result |
| --- | --- |
| [Normal stable PBS baseline](../pbs-independent-checks/) | Build and validator passed; installed Oriole ran on glibc 2.17 with 1,024 threaded parses. Both XML gates retain the same two callback assertions. |
| [Fresh local Ohm PGO bundle](local-ohm/README.md) | First build passed: three workspace compiler invocations and 288 generated parses per phase; generated callbacks agree. |
| Local C consumers | Existing shared-library and exact bundled-static integration consumers both passed on their first attempts. |
| Python tooling | 14 PGO tests and 6 bundle provenance tests, Ruff 0.16.6, ty 0.0.80, formatting and recipe checks passed; earlier failed tooling attempts are retained. |
| Workflow routing | YAML and shell syntax checked; inert-command probes verify the normal and PGO argument paths, including a profiler path containing spaces. |

## Selecting PGO

The [bundle recipe](../../../../integration/python-build-standalone/#optional-fresh-pgo-bundle) accepts `--pgo --llvm-profdata /absolute/matching/llvm-profdata`. Normal invocations retain the normal release build.

The PBS workflow selects PGO for pull-request head branches beginning `charlie/codex-oriole-pbs-pgo-`. Other PBS pull requests remain normal. Once the workflow exists on the default branch, manual dispatch can also select its boolean `pgo` input, which defaults to `false`. A single job-level flag controls tool preparation and bundle selection. PGO runs install stable Rust's matching LLVM tools and fetch locked dependencies before the offline build/train/merge/use pipeline.

The workflow keeps the strict XML failures visible while running the independent installed identity and glibc gates after a successful build. Its PGO evidence upload retains manifests, logs, generated inputs and records, and profiles. Cargo target caches and duplicate compiled libraries are excluded. The bundle manifest retains the exact static archive hash; its identity inside the produced PGO distribution remains a separate readback check.

## Source and evidence

The [local summary](local-ohm/summary.json) binds the exact bundle archive `2d1d5a592ec2debeadef526a45dc95d1adcc3e2b67c0e4b7ddad1c6878ede6e8`, compiler, profile, source and C-consumer records. Its [sealed evidence](local-ohm/evidence.tar.gz) and [member index](local-ohm/members.json.gz) retain source reviews, all Python-check attempts, raw build/training/profile results and C checks. The local archive retains an optional weak TLS reference; PBS's existing final linker wrap and actual glibc behavior still require the new installed PGO distribution trial.

That local build used the bridge patch on PR124 `4d603c7487ca93cc73dbd4acfe6fa5714ffc2151`. The publication branch was subsequently advanced to the test-only PR125 `144e69e08c5462217d54791374e579e3ef390a87`, with no parser runtime change. The two local C consumer runs used the earlier, explicitly preserved integration fixture; the newer nested-entity fixture is not claimed as part of those runs. The [normal PBS report](../pbs-independent-checks/report.json) identifies its own stable compiler, archive and installed test outcomes separately.

The [report](report.json), [workflow check record](workflow-check.json) and [executed check source](workflow-check.py) describe the routing probes and source boundaries. The [workflow review](workflow-independent-review.json) checks the opt-in routing, and the [archive review](archive-root-review.json) independently checks both evidence archives. [File hashes](files.json) identify the report's publication inputs. An installed stable PGO archive, its XML/allocator/glibc gates, and application measurements remain separate required evidence before assigning this configuration production readiness.

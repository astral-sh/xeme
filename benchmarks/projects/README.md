# Real-project XML benchmark corpus

The corpus covers six XML uses selected before timing: code-generation registries, protocol descriptions, build metadata, SVG artwork, UI trees and XSLT source. Inputs are original upstream files without repetition, concatenation, truncation or rewriting. Each project contributes one file; every file is retained with its original license/notice, exact commit and SHA256 in `corpus-manifest.json`.

| Project | Input | Bytes | License |
| --- | --- | ---: | --- |
| Khronos Vulkan | `xml/vk.xml` | 3,309,653 | Apache-2.0 OR MIT |
| Wayland | `protocol/wayland.xml` | 151,742 | MIT |
| Apache Maven | `pom.xml` | 47,826 | Apache-2.0 |
| Apache Batik | `samples/batikLogo.svg` | 16,607 | Apache-2.0 |
| GTK | `gtk/ui/gtkfilechooserwidget.ui` | 26,739 | LGPL-2.1-or-later |
| DocBook XSL | `xsl/fo/docbook.xsl` | 17,433 | DocBook permissive license |

The Wayland repository is a GitHub mirror of the upstream GitLab project. GTK's initially selected file chooser dialog was a 1,245-byte wrapper; its main file chooser widget replaced it before any parsing or timing. The wrapper, initial manifest and initial failed license-path fetches remain recorded. Corpus selection did not use performance results.

## Native parser measurement

`benchmarks/projects.py` uses the existing reviewed native C driver. Both libraries receive the same immutable byte stream, 4/64 KiB chunks and element/text handlers, with namespace processing independently disabled and enabled. Creation, callback registration, parsing, callback hashing and freeing are timed. Library loading, process startup and file I/O are excluded.

Every condition first compares complete normalized callbacks from the independent ctypes probe. Text fragments are coalesced; all other captured callback types remain exact. The complete callbacks independently derive the C driver's expected FNV-1a hash, element count and text-byte count. Every timed sample must match that result. Callback output, process failures and source/input/library hashes are retained.

The default is seven randomized process pairs with twenty parses per process and one discarded warmup. Reported speedups are medians of the paired Expat/Oriole process medians; a speedup below one is a slowdown. The geometric mean weights each project equally within a namespace/chunk group; it does not describe a project build's elapsed time.

## Matched CPython consumers

`benchmarks/build_project_consumers.py` compiles unmodified CPython 3.12.13 `pyexpat.c` and `_elementtree.c` twice, with identical compiler flags and headers, against the two parser libraries. No source adaptation or alternate allocator is applied. `benchmarks/python_projects.py` explicitly loads those extensions even when the interpreter has built-in XML modules. It verifies the resolved `XML_Parse` library using `dladdr` and the expected SHA256.

Two consumer workloads use each original XML file:

- ElementTree creates and destroys a complete tree, preserving expanded names, attributes, text, tail and child counts.
- pyexpat creates and destroys a Python event list through start/end/text handlers with namespace processing enabled.

Creation, feeds, finalization, callbacks and result destruction are timed. Process startup, imports, file loading, explicit garbage collection before each parse, and canonical output checks are excluded. Garbage collection remains enabled during parsing. Destruction is measured separately after checking the output; this warms tree/event cache lines equally but differs from application lifetimes. Every sample's complete canonical output must match the other library's output. The default is seven randomized process pairs with ten parses after one discarded warmup.

## Scope and limits

The native and Python measurements use real project inputs and realistic XML consumers. They exclude later project processing, such as Vulkan code generation, SVG rendering or XSLT transformation. The separate Wayland measurement below runs complete code-generation commands. External entity resolution is disabled; Batik's external DTD is skipped by both libraries. XML stylesheet includes remain data. No runtime network or external file loading occurs.

The Linux shared host has uncontrolled CPU frequency, host load and memory bandwidth. Timings are pinned to CPU 0 and execute sequentially. Other agents may build or profile on other CPUs. These conditions, exact commands, all raw samples, preflights and unchanged source/input/binary hashes are recorded in the reports. No workload is removed after seeing a timing result.

## Actual Wayland code generation

`benchmarks/wayland_project.py` measures the unmodified Wayland scanner on its actual protocol XML in `client-header` and `private-code` modes. The pinned `scanner.c` and `wayland-util.c` are compiled with identical `-O3` flags for each parser. The version header is generated from the upstream template using its declared 1.23.90 version. Optional libxml DTD validation is disabled identically; the ordinary Expat `XML_GetBuffer` / `XML_ParseBuffer` path is unchanged. Full generated output must match byte-for-byte.

This measurement includes process startup, library loading, input I/O, XML parsing, protocol semantic processing, code generation, output file writes and process exit. It excludes hashing the generated output. Seven randomized pairs each contain ten independent process runs; one initial warmup per engine/mode is discarded. This is a complete project command, covering more than the XML parser microbenchmarks, although it still represents one protocol file and two code-generation modes.

The [baseline evidence archive](https://github.com/astral-sh/oriole/blob/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/benchmarks/results/2026-09-10/real-project-baseline/README.md) includes `usage-evidence.json`, which records that Vulkan's pinned registry loader uses the same standard-library ElementTree API exercised by the Python consumer benchmark. `consumer-origin-audit.json` separately checks the actual function pointers in `pyexpat.expat_CAPI` and confirms ElementTree uses the loaded native accelerator; all parser, creation and destruction pointers resolve to the intended frozen parser library.

See the [baseline results](https://github.com/astral-sh/oriole/blob/fe31da9b4050dfc901aa2fbd1080cb558e1c9f3f/benchmarks/results/2026-09-10/real-project-baseline/README.md) and [reproduction commands](RERUN.md). Initial selection/fetch records and consumer-origin audits are retained in the evidence archive.

# Real-project XML benchmark corpus

The corpus covers six XML uses: code-generation registries, protocol descriptions,
build metadata, SVG artwork, UI trees and XSLT source. Each project contributes
one unmodified upstream file, selected before timing.
[`corpus-manifest.json`](corpus-manifest.json) records its commit, SHA-256 and
license or notice.

| Project | Input | Bytes | License |
| --- | --- | ---: | --- |
| Khronos Vulkan | `xml/vk.xml` | 3,309,653 | Apache-2.0 OR MIT |
| Wayland | `protocol/wayland.xml` | 151,742 | MIT |
| Apache Maven | `pom.xml` | 47,826 | Apache-2.0 |
| Apache Batik | `samples/batikLogo.svg` | 16,607 | Apache-2.0 |
| GTK | `gtk/ui/gtkfilechooserwidget.ui` | 26,739 | LGPL-2.1-or-later |
| DocBook XSL | `xsl/fo/docbook.xsl` | 17,433 | DocBook permissive license |

The Wayland repository is a GitHub mirror of the upstream GitLab project.

## Native parser measurement

`benchmarks/projects.py` uses the native C driver. Both libraries receive the same
input, 4/64 KiB chunks and element/text handlers, with namespace processing off
and on. Creation, callback registration, parsing, callback hashing and freeing
are timed. Library loading, process startup and file I/O are excluded.

Before timing each condition, a ctypes probe compares complete normalized
callbacks. Text fragments are coalesced; all other captured callback types must
match exactly. The probe derives the C driver's expected FNV-1a hash, element
count and text-byte count. Every timed sample must match that result. The runner
retains callback output, process failures and source/input/library hashes.

The default is seven randomized process pairs with twenty parses per process and one discarded warmup. Reported speedups are medians of the paired Expat/Xeme process medians; a speedup below one is a slowdown. The geometric mean weights each project equally within a namespace/chunk group; it does not describe a project build's elapsed time.

## Matched CPython consumers

`benchmarks/build_project_consumers.py` compiles unmodified CPython 3.12.13
`pyexpat.c` and `_elementtree.c` against each parser library, using the same
compiler flags, headers and default allocator. `benchmarks/python_projects.py`
loads those extensions even when the interpreter has built-in XML modules. It
verifies the resolved `XML_Parse` library using `dladdr` and the expected SHA-256.

Two consumer workloads use each original XML file:

- ElementTree creates and destroys a complete tree, preserving expanded names, attributes, text, tail and child counts.
- pyexpat creates and destroys a Python event list through start/end/text handlers with namespace processing enabled.

Creation, feeds, finalization, callbacks and result destruction are timed. Process startup, imports, file loading, explicit garbage collection before each parse, and canonical output checks are excluded. Garbage collection remains enabled during parsing. Destruction is measured separately after checking the output; this warms tree/event cache lines equally but differs from application lifetimes. Every sample's complete canonical output must match the other library's output. The default is seven randomized process pairs with ten parses after one discarded warmup.

## Scope and limits

The native and Python measurements exclude later project processing, such as Vulkan code generation, SVG rendering or XSLT transformation. The separate Wayland measurement below runs complete code-generation commands. External entity resolution is disabled; Batik's external DTD is skipped by both libraries. XML stylesheet includes remain data. No runtime network or external file loading occurs.

Reserve a CPU and execute timings sequentially. CPU affinity does not isolate frequency, host load or memory bandwidth on a shared host. Record host conditions alongside the commands, samples, preflights and source/input/binary hashes. Keep every workload in the results, including regressions.

## Wayland code generation

`benchmarks/wayland_project.py` measures the unmodified Wayland scanner on
`wayland.xml` in `client-header` and `private-code` modes. The pinned `scanner.c`
and `wayland-util.c` are compiled with identical `-O3` flags for each parser. The
version header uses the upstream template and declared version, 1.23.90. Optional
libxml DTD validation is disabled for both builds. Both use the scanner's
`XML_GetBuffer` / `XML_ParseBuffer` path, and their generated output must match
byte-for-byte.

This measurement includes process startup, library loading, input I/O, XML parsing, protocol semantic processing, code generation, output file writes and process exit. It excludes hashing the generated output. Seven randomized pairs each contain ten independent process runs; one initial warmup per engine/mode is discarded. It covers one protocol file and two code-generation modes.

See the [reproduction commands](RERUN.md).

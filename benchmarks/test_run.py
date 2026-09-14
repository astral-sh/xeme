"""Generated-input selection, retained manifests, and downstream failures."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import run


class GeneratedRunnerTests(unittest.TestCase):
    def test_generated_inputs_namespaces_and_failure_propagation(self):
        for namespaces in [False, True]:
            with (
                self.subTest(namespaces=namespaces),
                tempfile.TemporaryDirectory() as tmp,
            ):
                directory = Path(tmp)
                library = directory / "parser.so"
                library.touch()
                manifest = directory / "build.json"
                manifest.write_text('{"engine": "candidate"}\n')
                output = directory / "benchmark"
                arguments = [
                    "--library",
                    str(library),
                    "--output",
                    str(output),
                    "--size",
                    "2",
                    "--build-manifest",
                    str(manifest),
                ]
                if namespaces:
                    arguments.append("--namespaces")
                with (
                    patch.object(
                        run.ctypes.util, "find_library", return_value=str(library)
                    ),
                    patch.object(run.projects, "main", return_value=23) as shared,
                ):
                    self.assertEqual(run.main(arguments), 23)
                forwarded = shared.call_args.args[0]
                self.assertEqual(
                    forwarded[forwarded.index("--namespaces") + 1],
                    "on" if namespaces else "off",
                )
                self.assertEqual(
                    (output / "elements.xml").read_bytes().count(b"<item "), 2
                )
                corpus = json.loads((output / "corpus.json").read_text())
                self.assertEqual((corpus["kind"], corpus["size"]), ("generated", 2))
                self.assertEqual(
                    (output / "build-manifest-0.json").read_bytes(),
                    manifest.read_bytes(),
                )

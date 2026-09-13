"""A selected probe library must not call a process-global implementation."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


@unittest.skipUnless(hasattr(os, "RTLD_DEEPBIND"), "requires ELF deep binding")
class LibraryIsolationTests(unittest.TestCase):
    def test_internal_calls_use_the_selected_library(self) -> None:
        compiler = shutil.which("cc")
        if compiler is None:
            self.skipTest("requires a C compiler")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, source in {
                "global": "int xeme_probe_owner(void) { return 111; }",
                "selected": """
                    int xeme_probe_owner(void) { return 222; }
                    int xeme_probe_result(void) { return xeme_probe_owner(); }
                """,
            }.items():
                path = root / f"{name}.c"
                path.write_text(source)
                subprocess.run(
                    [
                        compiler,
                        "-shared",
                        "-fPIC",
                        str(path),
                        "-o",
                        str(path.with_suffix(".so")),
                    ],
                    check=True,
                    capture_output=True,
                    timeout=30,
                )
            probe = """
import ctypes
import sys
sys.path.insert(0, sys.argv[1])
from xml_abi import load_library
global_library = ctypes.CDLL(sys.argv[2], mode=ctypes.RTLD_GLOBAL)
loader = load_library if sys.argv[4] == "isolated" else ctypes.CDLL
selected = loader(sys.argv[3])
selected.xeme_probe_result.argtypes = []
selected.xeme_probe_result.restype = ctypes.c_int
print(selected.xeme_probe_result())
"""
            for mode, expected in [("ordinary", 111), ("isolated", 222)]:
                with self.subTest(mode=mode):
                    result = subprocess.run(
                        [
                            sys.executable,
                            "-I",
                            "-S",
                            "-c",
                            probe,
                            str(Path(__file__).resolve().parent),
                            str(root / "global.so"),
                            str(root / "selected.so"),
                            mode,
                        ],
                        check=True,
                        capture_output=True,
                        text=True,
                        timeout=30,
                    )
                    self.assertEqual(int(result.stdout), expected)


if __name__ == "__main__":
    unittest.main()

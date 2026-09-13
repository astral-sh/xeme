"""Route initial and fresh imports to the two locally built XML extensions.

The harness installs this file as sitecustomize.py beside those extensions.
Normal import machinery still honors blocked imports in sys.modules.
"""

import importlib
import importlib.util
import sys
import sysconfig
from pathlib import Path


class ConsumerExtensions:
    def __init__(self, root: Path) -> None:
        suffix = sysconfig.get_config_var("EXT_SUFFIX")
        self.paths = {
            name: root / (name + suffix) for name in ("pyexpat", "_elementtree")
        }

    def find_spec(self, fullname, path=None, target=None):
        filename = self.paths.get(fullname)
        if filename is None:
            return None
        return importlib.util.spec_from_file_location(fullname, filename)


root = Path(__file__).resolve().parent
sys.meta_path.insert(0, ConsumerExtensions(root))
for name in ("pyexpat", "_elementtree"):
    sys.modules.pop(name, None)
    module = importlib.import_module(name)
    assert module.__file__ is not None
    assert Path(module.__file__).resolve() == root / (
        name + sysconfig.get_config_var("EXT_SUFFIX")
    )

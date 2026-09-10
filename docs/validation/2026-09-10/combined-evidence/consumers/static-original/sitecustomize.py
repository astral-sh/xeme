
import importlib.util
from pathlib import Path
import sys
import sysconfig
root = Path(__file__).resolve().parent
for name in ('pyexpat', '_elementtree'):
    path = root / (name + sysconfig.get_config_var('EXT_SUFFIX'))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    assert Path(module.__file__).resolve() == path

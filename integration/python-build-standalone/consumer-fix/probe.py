"""One isolated child-creation failure; invoked by run.py, never imported."""

from __future__ import annotations

import ctypes
import importlib.util
import json
import os
import resource
import sys
from pathlib import Path


def main() -> None:
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_CPU, (10, 10))
    resource.setrlimit(resource.RLIMIT_AS, (1024**3, 1024**3))
    path = Path(sys.argv[1]).resolve()
    # PBS can contain a builtin pyexpat. Load this exact extension explicitly.
    spec = importlib.util.spec_from_file_location("pyexpat", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["pyexpat"] = module
    spec.loader.exec_module(module)
    assert module.__file__ is not None
    assert Path(module.__file__).resolve() == path
    shim = ctypes.CDLL(sys.argv[2])
    shim.xeme_forced_child_failures.restype = ctypes.c_uint
    parent = module.ParserCreate()
    # Retain surplus ownership so a single incorrect decrement cannot free it.
    retained = [parent] * 8
    before = sys.getrefcount(parent)
    failed = False
    try:
        parent.ExternalEntityParserCreate("")
    except MemoryError:
        failed = True
    after = sys.getrefcount(parent)
    calls = shim.xeme_forced_child_failures()
    print(
        json.dumps(
            {
                "extension": str(path),
                "version": module.EXPAT_VERSION,
                "refcount_before": before,
                "refcount_after": after,
                "delta": after - before,
                "memory_error": failed,
                "shim_calls": calls,
                "retained_references": len(retained),
            }
        ),
        flush=True,
    )
    # Avoid interpreter teardown after any observed refcount corruption.
    os._exit(0 if failed and before == after and calls == 1 else 2)


if __name__ == "__main__":
    main()

"""Fail closed when a fresh test import loses a pinned XML extension."""

import hashlib
import importlib
import json
import sys
import sysconfig
from pathlib import Path


def main() -> None:
    if not __debug__:
        raise RuntimeError("extension origin checks require Python assertions enabled")
    root = Path(sys.argv[1]).resolve()
    # These helpers come from the pinned CPython source tree supplied by the
    # runner; installed interpreters and type stubs need not include its tests.
    import_fresh_module = importlib.import_module(
        "test.support.import_helper"
    ).import_fresh_module
    origins = {}
    for name in ("pyexpat", "_elementtree"):
        expected = root / (name + sysconfig.get_config_var("EXT_SUFFIX"))
        for label, module in (
            ("initial", importlib.import_module(name)),
            ("fresh", import_fresh_module(name)),
        ):
            assert module is not None, (name, label)
            assert module.__file__ is not None
            assert module.__spec__ is not None and module.__spec__.origin is not None
            assert Path(module.__file__).resolve() == expected, (name, label)
            assert Path(module.__spec__.origin).resolve() == expected, (name, label)
            origins[f"{label}:{name}"] = {
                "path": str(expected),
                "sha256": hashlib.sha256(expected.read_bytes()).hexdigest(),
            }

    # Execute the unchanged accelerator test module's two import_fresh_module
    # calls, including the historical cElementTree alias path.
    test_xml_etree_c = importlib.import_module("test.test_xml_etree_c")

    assert test_xml_etree_c.cET is not None
    assert test_xml_etree_c.cET_alias is not None
    for module in (test_xml_etree_c.cET, test_xml_etree_c.cET_alias):
        parser = module.XMLParser()
        parser.feed("<r/>")
        assert parser.close().tag == "r"

    # Pure-Python suites deliberately block the accelerator. A finder must not
    # override the normal sys.modules[name] = None contract.
    pure = import_fresh_module("xml.etree.ElementTree", blocked=["_elementtree"])
    assert pure is not None
    assert pure.Element is not test_xml_etree_c.cET.Element
    for name in ("pyexpat", "_elementtree"):
        saved = sys.modules.get(name)
        # CPython deliberately supports None sentinels despite the narrower
        # typeshed annotation for sys.modules.
        sys.modules[name] = None  # ty: ignore[invalid-assignment]
        try:
            try:
                importlib.import_module(name)
            except ModuleNotFoundError:
                pass
            else:
                raise AssertionError(f"blocked import succeeded: {name}")
        finally:
            if saved is None:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = saved
    print(
        json.dumps(
            {
                "status": "passed",
                "origins": origins,
                "accelerator_imports": ["cET", "cET_alias"],
                "pure_python_and_blocked_imports": "passed",
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()

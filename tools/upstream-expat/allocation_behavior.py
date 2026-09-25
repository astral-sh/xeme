"""Adapt pinned Expat allocation tests without requiring its allocation schedule."""

from __future__ import annotations

import difflib
import hashlib
import re
from pathlib import Path

REVISION = "4b3f0b06f39fb5529cead381694f8929901bc273"
RETRY_CEILING = 512
EXTRA_TESTS = {
    "test_mem_api_cycle",
    "test_mem_api_unlimited",
    "test_bypass_heuristic_when_close_to_bufsize",
}
INVENTORY_SHA256 = "7407dc73a762540b15880e14b790ff951f9eafa0c87e077896a8ed8d1483f57c"
SOURCE_SHA256 = {
    "alloc_tests.c": "d7285ac29f476045b884bdd8fe4d5b7cdf7316a9f4dedbb54d5497f1f6ddde15",
    "nsalloc_tests.c": "f51a351d8cde0d3ba774284245d65d0572420af36511d8b7c33cbbff39eb9b48",
    "handlers.c": "3435e02418febb9966e5822e3715fa87c0a6b4a1bb098afb0ee97834ddce9cd8",
    "common.c": "799da6b4ff129f60ed81ebd3a126ce840cb4c78b4a3b8ef9c16dc5acec5354f1",
    "basic_tests.c": "a58b2acf6a9d9017b349255641aa2f84e26596d77fa54796b74fc83745e6b92e",
    "misc_tests.c": "8795150cfbde2e1511a332dedcff3f9dc276f5172f2c58f0b495cb1cd7fd0ab9",
}


def _digest(text: str) -> str:
    return hashlib.sha256(text.encode()).hexdigest()


def selected_tests(public_tests: set[str]) -> list[str]:
    """Select the complete public allocation suites and the deferral-growth test."""
    selected = sorted(
        name
        for name in public_tests
        if name.startswith(("test_alloc_", "test_nsalloc_")) or name in EXTRA_TESTS
    )
    if len(selected) != 84 or _digest("\n".join(selected) + "\n") != INVENTORY_SHA256:
        raise ValueError(f"pinned allocation test inventory changed: {selected}")
    return selected


NESTED_HANDLER = """/* Sweep child parsing independently of parent/child creation costs. */
static int xeme_nested_budget;
static enum XML_Status xeme_nested_status;

static int XMLCALL
xeme_nested_entity_handler(XML_Parser parser, const XML_Char *context,
                           const XML_Char *base, const XML_Char *systemId,
                           const XML_Char *publicId) {
  ExtFaults *fault = XML_GetUserData(parser);
  UNUSED_P(base);
  UNUSED_P(systemId);
  UNUSED_P(publicId);
  XML_Parser child = XML_ExternalEntityParserCreate(parser, context, NULL);
  if (child == NULL)
    fail("Could not create external entity parser without injected failures");
  g_allocation_count = xeme_nested_budget;
  xeme_nested_status = _XML_Parse_SINGLE_BYTES(
      child, fault->parse_text, (int)strlen(fault->parse_text), XML_TRUE);
  g_allocation_count = ALLOC_ALWAYS_SUCCEED;
  if (xeme_nested_status == XML_STATUS_ERROR) {
    if (XML_GetErrorCode(child) != fault->error)
      xml_failure(child);
  } else if (xeme_nested_status != XML_STATUS_OK) {
    fail("Unexpected child parse status");
  }
  XML_ParserFree(child);
  return xeme_nested_status;
}

"""

NESTED_SWEEP = """  int failures = 0;
  int i;
  for (i = 0; i < 512; i++) {
    g_allocation_count = ALLOC_ALWAYS_SUCCEED;
    xeme_nested_budget = i;
    xeme_nested_status = XML_STATUS_SUSPENDED; /* handler not called yet */
    XML_SetUserData(g_parser, &test_data);
    XML_SetParamEntityParsing(g_parser, XML_PARAM_ENTITY_PARSING_ALWAYS);
    XML_SetExternalEntityRefHandler(g_parser, xeme_nested_entity_handler);
    const enum XML_Status status
        = _XML_Parse_SINGLE_BYTES(g_parser, text, (int)strlen(text), XML_TRUE);
    if (xeme_nested_status == XML_STATUS_OK) {
      if (status != XML_STATUS_OK)
        xml_failure(g_parser);
      break;
    }
    if (xeme_nested_status != XML_STATUS_ERROR)
      fail("Nested entity handler was not called");
    if (status != XML_STATUS_ERROR
        || XML_GetErrorCode(g_parser) != XML_ERROR_EXTERNAL_ENTITY_HANDLING)
      fail("Entity allocation failure not noted");
    failures++;
    alloc_teardown();
    alloc_setup();
  }
  assert_true(failures > 0);
  assert_true(i < 512);"""

CREATE_SWEEP = """START_TEST(test_alloc_reset_after_external_entity_parser_create_fail) {
  const char *const text = "<!DOCTYPE doc SYSTEM 'foo'><doc/>";
  int state[2]; /* allocation budget, whether child creation succeeded */
  int failures = 0;
  int i;
  for (i = 0; i < 512; i++) {
    state[0] = i;
    state[1] = -1;
    XML_SetUserData(g_parser, state);
    XML_SetExternalEntityRefHandler(
        g_parser, external_entity_parser_create_alloc_fail_handler);
    XML_SetParamEntityParsing(g_parser, XML_PARAM_ENTITY_PARSING_ALWAYS);
    if (_XML_Parse_SINGLE_BYTES(g_parser, text, (int)strlen(text), XML_TRUE)
        != XML_STATUS_ERROR)
      fail("Call to parse was expected to fail");
    if (XML_GetErrorCode(g_parser) != XML_ERROR_EXTERNAL_ENTITY_HANDLING)
      fail("Call to parse was expected to fail from the external entity handler");
    assert_true(XML_ParserReset(g_parser, NULL) == XML_TRUE);
    if (state[1] == 1)
      break;
    assert_true(state[1] == 0);
    failures++;
  }
  assert_true(failures > 0);
  assert_true(i < 512);
}
END_TEST"""


def adapt_sources(directory: Path) -> dict:
    """Adapt clean copied test sources before the regular public-API adapters."""
    originals = {name: (directory / name).read_text() for name in SOURCE_SHA256}
    for name, source in originals.items():
        if _digest(source) != SOURCE_SHA256[name]:
            raise ValueError(f"allocation adapter expected pristine pinned {name}")
    sources = originals.copy()
    edits: list[dict] = []

    def replace(name: str, old: str, new: str, reason: str, count: int = 1) -> None:
        actual = sources[name].count(old)
        if actual != count:
            raise ValueError(
                f"{name}: expected {count} matches for {reason}, got {actual}"
            )
        sources[name] = sources[name].replace(old, new)
        edits.append({"file": name, "reason": reason, "replacements": count})

    def substitute(name: str, pattern: str, replacement: str, reason: str) -> None:
        sources[name], count = re.subn(pattern, replacement, sources[name])
        if not count:
            raise ValueError(f"{name}: no matches for {reason}")
        edits.append({"file": name, "reason": reason, "replacements": count})

    for name in sources:
        sources[name] = '#include "allocation_tracker.h"\n' + sources[name]
        edits.append({"file": name, "reason": "declare audit hooks", "replacements": 1})

    # These two branches assert opposite allocation schedules. Both retain the
    # same requirement that parsing eventually succeeds within the retry budget.
    replace(
        "nsalloc_tests.c",
        """#if XML_GE == 1
  assert_true(
      i == 0); // because expat_realloc relies on expat_malloc to some extent
#else
  if (i == 0)
    fail("Parsing worked despite failing reallocations");
  else if (i == max_realloc_count)
    fail("Parsing failed at max reallocation count");
#endif""",
        """  if (i == max_realloc_count)
    fail("Parsing failed at max reallocation count");""",
        "permit either realloc schedule for namespaced attributes",
    )
    for name in ("alloc_tests.c", "nsalloc_tests.c", "handlers.c", "misc_tests.c"):
        substitute(
            name,
            r"(const (?:unsigned(?: int)?|int) "
            r"(?:max_alloc_count|max_allocation_count|alloc_test_max_repeats|"
            r"max_realloc_count) = )\d+;",
            rf"\g<1>{RETRY_CEILING};",
            "raise allocation retry ceilings without removing eventual success",
        )
        substitute(
            name,
            r'(?m)^( +)if \(i == 0\)\n\1  fail\("[^"\n]*"\);\n'
            r"\1(?:else )?if \(i == (\w+)\)",
            r"\1if (i == \2)",
            "permit success without an implementation-specific allocation",
        )
    replace(
        "handlers.c",
        """    if (i == 0) {
      fail("Second external parser unexpectedly created");
      XML_ParserFree(new_parser);
      return XML_STATUS_ERROR;
    } else if (i == max_alloc_count) {""",
        "    if (i == max_alloc_count) {",
        "permit identical allocation counts for two external parser creations",
    )

    replace(
        "nsalloc_tests.c",
        """  g_allocation_count = 0;
  if (XML_ParseBuffer(g_parser, 0, XML_FALSE) != XML_STATUS_ERROR)
    fail("Pre-init XML_ParseBuffer not faulted");
  if (XML_GetErrorCode(g_parser) != XML_ERROR_NO_MEMORY)
    fail("Pre-init XML_ParseBuffer faulted for wrong reason");""",
        """  g_allocation_count = 0;
  const enum XML_Status empty_status = XML_ParseBuffer(g_parser, 0, XML_FALSE);
  if (empty_status == XML_STATUS_ERROR) {
    if (XML_GetErrorCode(g_parser) != XML_ERROR_NO_MEMORY)
      fail("Pre-init XML_ParseBuffer faulted for wrong reason");
  } else if (empty_status != XML_STATUS_OK
             || XML_GetErrorCode(g_parser) != XML_ERROR_NONE) {
    fail("Unexpected empty XML_ParseBuffer status");
  }""",
        "allow empty parsing to need no allocation; retain all state/error checks",
    )
    replace(
        "alloc_tests.c",
        "START_TEST(test_alloc_nested_entities) {",
        NESTED_HANDLER + "START_TEST(test_alloc_nested_entities) {",
        "add a dedicated nested-entity fault handler",
    )
    replace(
        "alloc_tests.c",
        """  /* Causes an allocation error in a nested storeEntityValue() */
  g_allocation_count = 12;
  XML_SetUserData(g_parser, &test_data);
  XML_SetParamEntityParsing(g_parser, XML_PARAM_ENTITY_PARSING_ALWAYS);
  XML_SetExternalEntityRefHandler(g_parser, external_entity_faulter);
  expect_failure(text, XML_ERROR_EXTERNAL_ENTITY_HANDLING,
                 "Entity allocation failure not noted");""",
        NESTED_SWEEP,
        "sweep nested child parsing; retain child and parent error propagation",
    )
    # A DOTALL pattern is scoped to this single named test.
    substitute(
        "alloc_tests.c",
        r"(?s)START_TEST\(test_alloc_reset_after_external_entity_parser_create_fail\)"
        r" \{.*?\nEND_TEST",
        CREATE_SWEEP,
        "exercise parent reset after every child-creation failure budget",
    )
    replace(
        "handlers.c",
        """  // The following number intends to fail the upcoming allocation in line
  // "parser->m_protocolEncodingName = copyString(encodingName,
  // &(parser->m_mem));" in function parserInit.
  g_allocation_count = 3;

  const XML_Char *const encodingName = XCS("UTF-8"); // needs something non-NULL
  const XML_Parser ext_parser
      = XML_ExternalEntityParserCreate(parser, context, encodingName);
  if (ext_parser != NULL)
    fail(
        "Call to XML_ExternalEntityParserCreate was expected to fail out-of-memory");

  g_allocation_count = ALLOC_ALWAYS_SUCCEED;""",
        """  int *state = XML_GetUserData(parser);
  g_allocation_count = state[0];
  const XML_Char *const encodingName = XCS("UTF-8");
  const XML_Parser ext_parser
      = XML_ExternalEntityParserCreate(parser, context, encodingName);
  g_allocation_count = ALLOC_ALWAYS_SUCCEED;
  state[1] = (ext_parser != NULL);
  XML_ParserFree(ext_parser);""",
        "replace fixed child-creation allocation point with caller's sweep budget",
    )

    substitute(
        "basic_tests.c",
        r"(?s)        // Now, check that we've had a buffer allocation.*?"
        r"(?=        // fill data until the big token is actually parsed)",
        "",
        "remove required buffer allocation size while retaining parse progress",
    )
    replace(
        "basic_tests.c",
        "          const size_t alloc_before = g_totalAlloc;\n",
        "",
        "remove allocation-growth measurement local",
    )
    substitute(
        "basic_tests.c",
        r"(?s)          // since all the bytes of the big token.*?"
        r"          assert_true\(g_totalAlloc - alloc_before < 4096\);\n",
        "",
        "remove Expat's exact buffer-growth ceiling",
    )
    replace(
        "basic_tests.c",
        "        // test-the-test: was our alloc even called?\n"
        "        assert_true(g_totalAlloc > 0);\n",
        "",
        "permit deferral without additional allocation",
    )

    for name in ("alloc_tests.c", "nsalloc_tests.c"):
        replace(
            name,
            "{duff_allocator, duff_reallocator, free}",
            "{duff_allocator, duff_reallocator, xeme_audit_free}",
            "track frees from injected-failure memory suites",
        )
        replace(
            name,
            "  basic_teardown();\n}",
            "  basic_teardown();\n  xeme_audit_checkpoint();\n}",
            "check ownership after every allocation-test teardown",
        )
    replace(
        "misc_tests.c",
        "{duff_allocator, realloc, free}",
        "{duff_allocator, xeme_audit_realloc, xeme_audit_free}",
        "keep all uses of the shared duff allocator on the same memory suite",
        count=2,
    )
    for counter, is_realloc in (("g_allocation_count", 0), ("g_reallocation_count", 1)):
        replace(
            "common.c",
            f"  if ({counter} == 0)\n    return NULL;",
            f"  if ({counter} == 0) {{\n    xeme_audit_failure({is_realloc});\n"
            "    return NULL;\n  }",
            f"record injected {'reallocation' if is_realloc else 'allocation'} failures",
        )
    replace(
        "common.c",
        "  return malloc(size);",
        "  return xeme_audit_malloc(size);",
        "track successful injected-suite allocations",
    )
    for name in ("common.c", "basic_tests.c"):
        replace(
            name,
            "  return realloc(ptr, size);",
            "  return xeme_audit_realloc(ptr, size);",
            "track successful reallocation ownership",
        )
    replace(
        "basic_tests.c",
        "      counting_malloc,\n      counting_realloc,\n      free,",
        "      counting_malloc,\n      counting_realloc,\n      xeme_audit_free,",
        "track frees in deferral memory suite",
    )

    patch = "".join(
        "".join(
            difflib.unified_diff(
                originals[name].splitlines(keepends=True),
                sources[name].splitlines(keepends=True),
                fromfile=f"a/{name}",
                tofile=f"b/{name}",
            )
        )
        for name in sorted(sources)
    )
    for name, text in sources.items():
        (directory / name).write_text(text)
    patch_path = directory.parent / "allocation-behavior.patch"
    patch_path.write_text(patch)
    return {
        "schema_version": 1,
        "source_revision": REVISION,
        "expected_tests": 84,
        "inventory_sha256": INVENTORY_SHA256,
        "retry_ceiling": RETRY_CEILING,
        "default_constructor_allocator": "tracked non-injecting suite via XML_ParserCreate_MM",
        "scope": "83 public allocation suite tests plus deferral buffer-growth test",
        "preserved_checks": [
            "bounded eventual success under allocation failure injection",
            "upstream callback data and handler flags",
            "empty-buffer, suspend/resume and finished-parser errors",
            "nested child NO_MEMORY and parent EXTERNAL_ENTITY_HANDLING",
            "parent reset after external child creation failures",
            "deferral parse progress and final element count across 504 size combinations",
            "custom allocator ownership at teardown",
        ],
        "source_sha256": SOURCE_SHA256.copy(),
        "adapted_sha256": {name: _digest(text) for name, text in sources.items()},
        "edits": edits,
        "patch": {"path": str(patch_path), "sha256": _digest(patch)},
    }

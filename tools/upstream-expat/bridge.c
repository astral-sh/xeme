#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#define XEME_BRIDGE_IMPLEMENTATION
#include "bridge.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Preserve the legacy API coverage while compiling against the new header. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif
int xeme_test_set_hash_salt(XML_Parser parser, unsigned long salt) {
  return XML_SetHashSalt(parser, salt);
}
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#endif

#ifdef XEME_ALLOCATION_BEHAVIOR
#include "allocation_tracker.h"

/* Diagnostic default constructors retain unlimited allocation while exposing
 * ownership to the audit. Otherwise memory-API tests would track only their
 * unrelated setup parser, leaving the parsers used by their bodies unobserved. */
static const XML_Memory_Handling_Suite audit_default_memory_suite = {
    xeme_audit_malloc, xeme_audit_realloc, xeme_audit_free};

/* Retry loops often check only ERROR. A denial during this exact call must
 * report OOM, or the parent-side external-entity error that propagates it. */
static enum XML_Status
check_allocation_error(XML_Parser parser, enum XML_Status status, size_t before) {
  if (status == XML_STATUS_ERROR && xeme_audit_failure_count() != before) {
    enum XML_Error error = XML_GetErrorCode(parser);
    if (error != XML_ERROR_NO_MEMORY && error != XML_ERROR_EXTERNAL_ENTITY_HANDLING) {
      fprintf(stderr, "XEME_ALLOCATION_ERROR\tinjected failure reported wrong parse error"
                      "\terror=%d\n", (int)error);
      fflush(NULL);
      _Exit(102);
    }
  }
  return status;
}

enum XML_Status
xeme_test_parse(XML_Parser parser, const char *text, int length, int final_input) {
  size_t before = xeme_audit_failure_count();
  enum XML_Status status = XML_Parse(parser, text, length, final_input);
  return check_allocation_error(parser, status, before);
}

enum XML_Status
xeme_test_parse_buffer(XML_Parser parser, int length, int final_input) {
  size_t before = xeme_audit_failure_count();
  enum XML_Status status = XML_ParseBuffer(parser, length, final_input);
  return check_allocation_error(parser, status, before);
}
#endif

/* This is harness state, never a substitute for an implementation counter. */
XML_Bool g_reparseDeferralEnabledDefault = XML_TRUE;
static struct {
  tcase_test_function function;
  const char *name;
} names[4096];
static size_t names_count;

void xeme_register_test(tcase_test_function function, const char *name) {
  if (names_count == sizeof(names) / sizeof(names[0]))
    abort();
  names[names_count].function = function;
  names[names_count++].name = name;
}

const char *xeme_test_name(tcase_test_function function) {
  for (size_t i = 0; i < names_count; ++i)
    if (names[i].function == function)
      return names[i].name;
  fprintf(stderr, "unregistered test function\n");
  abort();
}

int xeme_context_enabled(int chunk, int deferral) {
  const char *chunks = getenv("XEME_CHUNK_MASK");
  const char *modes = getenv("XEME_DEFERRAL_MASK");
  return (!chunks || (atoi(chunks) & (1 << chunk)))
         && (!modes || (atoi(modes) & (1 << deferral)));
}

void xeme_print_library(void) {
  Dl_info info;
  if (!dladdr((void *)XML_Parse, &info) || !info.dli_fname)
    abort();
  printf("XEME_LIBRARY\t%s\n", info.dli_fname);
}

static XML_Parser set_defaults(XML_Parser parser) {
  if (parser)
    XML_SetReparseDeferralEnabled(parser, g_reparseDeferralEnabledDefault);
  return parser;
}

XML_Parser xeme_test_create(const XML_Char *encoding) {
#ifdef XEME_ALLOCATION_BEHAVIOR
  return set_defaults(XML_ParserCreate_MM(encoding, &audit_default_memory_suite, NULL));
#else
  return set_defaults(XML_ParserCreate(encoding));
#endif
}
XML_Parser xeme_test_create_ns(const XML_Char *encoding, XML_Char separator) {
#ifdef XEME_ALLOCATION_BEHAVIOR
  return set_defaults(XML_ParserCreate_MM(encoding, &audit_default_memory_suite,
                                        &separator));
#else
  return set_defaults(XML_ParserCreateNS(encoding, separator));
#endif
}
XML_Parser xeme_test_create_mm(const XML_Char *encoding,
                               const XML_Memory_Handling_Suite *suite,
                               const XML_Char *separator) {
  return set_defaults(XML_ParserCreate_MM(encoding, suite, separator));
}
XML_Bool xeme_test_reset(XML_Parser parser, const XML_Char *encoding) {
  XML_Bool result = XML_ParserReset(parser, encoding);
  if (result)
    set_defaults(parser);
  return result;
}

int xeme_test_enabled(const char *name) {
  const char *selected = getenv("XEME_SELECTED_TESTS");
  if (!selected || !*selected)
    return 1;
  size_t length = strlen(name);
  while (*selected) {
    const char *end = strchr(selected, ',');
    size_t span = end ? (size_t)(end - selected) : strlen(selected);
    if (span == length && memcmp(selected, name, length) == 0)
      return 1;
    if (!end)
      break;
    selected = end + 1;
  }
  return 0;
}

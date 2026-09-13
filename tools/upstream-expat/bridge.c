#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#define XEME_BRIDGE_IMPLEMENTATION
#include "bridge.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

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
  return set_defaults(XML_ParserCreate(encoding));
}
XML_Parser xeme_test_create_ns(const XML_Char *encoding, XML_Char separator) {
  return set_defaults(XML_ParserCreateNS(encoding, separator));
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

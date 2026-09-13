/* Test adapter only: apply test defaults through the public Expat API. */
#ifndef ORIOLE_TEST_BRIDGE_H
#define ORIOLE_TEST_BRIDGE_H
#include "expat.h"
#include "minicheck.h"

void oriole_register_test(tcase_test_function function, const char *name);
const char *oriole_test_name(tcase_test_function function);
int oriole_context_enabled(int chunk, int deferral);
void oriole_print_library(void);
int oriole_test_enabled(const char *name);
XML_Parser oriole_test_create(const XML_Char *encoding);
XML_Parser oriole_test_create_ns(const XML_Char *encoding, XML_Char separator);
XML_Parser oriole_test_create_mm(const XML_Char *encoding,
                               const XML_Memory_Handling_Suite *suite,
                               const XML_Char *separator);
XML_Bool oriole_test_reset(XML_Parser parser, const XML_Char *encoding);

#ifndef ORIOLE_BRIDGE_IMPLEMENTATION
#define XML_ParserCreate oriole_test_create
#define XML_ParserCreateNS oriole_test_create_ns
#define XML_ParserCreate_MM oriole_test_create_mm
#define XML_ParserReset oriole_test_reset
#endif
#endif

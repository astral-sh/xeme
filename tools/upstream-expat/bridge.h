/* Test adapter only: apply test defaults through the public Expat API. */
#ifndef XEME_TEST_BRIDGE_H
#define XEME_TEST_BRIDGE_H
#include "expat.h"
#include "minicheck.h"

void xeme_register_test(tcase_test_function function, const char *name);
const char *xeme_test_name(tcase_test_function function);
int xeme_context_enabled(int chunk, int deferral);
void xeme_print_library(void);
int xeme_test_enabled(const char *name);
XML_Parser xeme_test_create(const XML_Char *encoding);
XML_Parser xeme_test_create_ns(const XML_Char *encoding, XML_Char separator);
XML_Parser xeme_test_create_mm(const XML_Char *encoding,
                               const XML_Memory_Handling_Suite *suite,
                               const XML_Char *separator);
XML_Bool xeme_test_reset(XML_Parser parser, const XML_Char *encoding);
int xeme_test_set_hash_salt(XML_Parser parser, unsigned long salt);

#ifdef XEME_ALLOCATION_BEHAVIOR
enum XML_Status xeme_test_parse(XML_Parser parser, const char *text, int length,
                               int final_input);
enum XML_Status xeme_test_parse_buffer(XML_Parser parser, int length,
                                      int final_input);
#endif

#ifndef XEME_BRIDGE_IMPLEMENTATION
#define XML_ParserCreate xeme_test_create
#define XML_ParserCreateNS xeme_test_create_ns
#define XML_ParserCreate_MM xeme_test_create_mm
#define XML_ParserReset xeme_test_reset
/* Expat 2.8.5 uses a private helper to avoid its public deprecation warning. */
#define xmlSetHashSalt xeme_test_set_hash_salt
#ifdef XEME_ALLOCATION_BEHAVIOR
#define XML_Parse xeme_test_parse
#define XML_ParseBuffer xeme_test_parse_buffer
#endif
#endif
#endif

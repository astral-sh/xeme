/* Diagnostic interposer: exercise CPython's NULL child-parser cleanup path. */
#include "expat.h"

static unsigned int calls;

XML_Parser XMLCALL
XML_ExternalEntityParserCreate(XML_Parser parser, const XML_Char *context,
                               const XML_Char *encoding) {
  (void)parser;
  (void)context;
  (void)encoding;
  calls++;
  return NULL;
}

unsigned int oriole_forced_child_failures(void) { return calls; }

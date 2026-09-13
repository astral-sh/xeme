#define XEME_BRIDGE_IMPLEMENTATION
#include "bridge.h"
#include "allocation_tracker.h"
#include <assert.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>

static int inject, queried;
static enum XML_Status result;
static enum XML_Error reported;
static const XML_Char *expected_encoding;
static int expected_namespace, mm_calls, defaults_calls, parser_marker;
static XML_Char expected_separator;

XML_Parser XML_ParserCreate_MM(const XML_Char *encoding,
                              const XML_Memory_Handling_Suite *suite,
                              const XML_Char *separator) {
  assert(encoding == expected_encoding);
  assert((separator != NULL) == expected_namespace);
  if (separator) assert(*separator == expected_separator);
  assert(suite && suite->malloc_fcn == xeme_audit_malloc
         && suite->realloc_fcn == xeme_audit_realloc
         && suite->free_fcn == xeme_audit_free);
  void *pointer = suite->malloc_fcn(17);
  assert(pointer);
  pointer = suite->realloc_fcn(pointer, 31);
  assert(pointer);
  suite->free_fcn(pointer);
  mm_calls++;
  return (XML_Parser)&parser_marker;
}

XML_Bool XML_SetReparseDeferralEnabled(XML_Parser parser, XML_Bool enabled) {
  assert(parser == (XML_Parser)&parser_marker && enabled == XML_TRUE);
  defaults_calls++;
  return XML_TRUE;
}

static void check_constructors(void) {
  size_t before = xeme_audit_failure_count();
  expected_encoding = NULL;
  assert(xeme_test_create(NULL) == (XML_Parser)&parser_marker);
  expected_encoding = "UTF-8";
  assert(xeme_test_create(expected_encoding) == (XML_Parser)&parser_marker);
  expected_namespace = 1;
  expected_separator = '|';
  assert(xeme_test_create_ns(expected_encoding, '|') == (XML_Parser)&parser_marker);
  expected_separator = '\0';
  assert(xeme_test_create_ns(expected_encoding, '\0') == (XML_Parser)&parser_marker);
  assert(mm_calls == 4 && defaults_calls == 4);
  assert(xeme_audit_failure_count() == before);
  xeme_audit_checkpoint();
}

enum XML_Status XML_Parse(XML_Parser parser, const char *text, int length, int final_input) {
  (void)parser; (void)text; (void)length; (void)final_input;
  if (inject) xeme_audit_failure(0);
  return result;
}
enum XML_Status XML_ParseBuffer(XML_Parser parser, int length, int final_input) {
  return XML_Parse(parser, NULL, length, final_input);
}
enum XML_Error XML_GetErrorCode(XML_Parser parser) {
  (void)parser;
  queried++;
  return reported;
}

int main(void) {
  check_constructors();
  const struct { int inject; enum XML_Status status; enum XML_Error error; int exit_code; } cases[] = {
    {1, XML_STATUS_ERROR, XML_ERROR_NO_MEMORY, 0},
    {1, XML_STATUS_ERROR, XML_ERROR_EXTERNAL_ENTITY_HANDLING, 0},
    {1, XML_STATUS_ERROR, XML_ERROR_SYNTAX, 102},
    {0, XML_STATUS_ERROR, XML_ERROR_SYNTAX, 0},
    {1, XML_STATUS_OK, XML_ERROR_SYNTAX, 0},
    {1, XML_STATUS_SUSPENDED, XML_ERROR_NONE, 0},
    {0, XML_STATUS_ERROR, XML_ERROR_NO_MEMORY, 0},
  };
  for (int buffered = 0; buffered < 2; ++buffered) {
    for (size_t index = 0; index < sizeof(cases) / sizeof(cases[0]); ++index) {
      fflush(NULL);
      pid_t child = fork();
      assert(child >= 0);
      if (!child) {
        alarm(5);
        inject = cases[index].inject;
        result = cases[index].status;
        reported = cases[index].error;
        /* A denial before entry must not be mistaken for a new denial. */
        xeme_audit_failure(1);
        size_t before = xeme_audit_failure_count();
        enum XML_Status actual = buffered ? xeme_test_parse_buffer(NULL, 0, 1)
                                          : xeme_test_parse(NULL, "", 0, 1);
        assert(actual == result);
        assert(xeme_audit_failure_count() == before + (size_t)inject);
        assert(queried == (inject && result == XML_STATUS_ERROR));
        _Exit(0);
      }
      int status;
      assert(waitpid(child, &status, 0) == child);
      assert(WIFEXITED(status) && WEXITSTATUS(status) == cases[index].exit_code);
    }
  }
  puts("allocation bridge self-checks passed: 14 child scenarios, 4 constructors");
  return 0;
}

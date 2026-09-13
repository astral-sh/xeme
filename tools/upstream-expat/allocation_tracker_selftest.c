/* Standalone checks for the diagnostic helper; no XML parser is linked. */
#include "allocation_tracker.h"

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static void
valid_ownership(void) {
  unsigned char *first = xeme_audit_malloc(32);
  void *second = xeme_audit_malloc(17);
  void *tail = xeme_audit_malloc(8);
  assert(first && second && tail);
  memset(first, 0x3c, 32);
  xeme_audit_free(tail);
  tail = xeme_audit_malloc(9);
  assert(tail);
  xeme_audit_free(second);
  xeme_audit_free(tail);

  unsigned char *grown = xeme_audit_realloc(first, 128);
  assert(grown);
  for (size_t index = 0; index < 32; ++index)
    assert(grown[index] == 0x3c);
  /* A real failed realloc must keep both the bytes and tracked owner intact. */
  assert(xeme_audit_realloc(grown, SIZE_MAX) == NULL);
  for (size_t index = 0; index < 32; ++index)
    assert(grown[index] == 0x3c);
  xeme_audit_failure(1);
  unsigned char *shrunk = xeme_audit_realloc(grown, 16);
  assert(shrunk);
  for (size_t index = 0; index < 16; ++index)
    assert(shrunk[index] == 0x3c);
  assert(xeme_audit_realloc(shrunk, 0) == NULL);

  void *fresh = xeme_audit_realloc(NULL, 24);
  assert(fresh);
  xeme_audit_free(fresh);
  xeme_audit_free(xeme_audit_malloc(0));
  xeme_audit_free(NULL);
  xeme_audit_failure(0);
  xeme_audit_checkpoint();
  xeme_audit_finish();
}

static void
leak(void) {
  assert(xeme_audit_malloc(7));
  xeme_audit_checkpoint();
}

static void
double_free(void) {
  void *pointer = xeme_audit_malloc(7);
  assert(pointer);
  xeme_audit_free(pointer);
  xeme_audit_free(pointer);
}

static void
foreign_free(void) {
  void *pointer = malloc(7);
  assert(pointer);
  xeme_audit_free(pointer);
}

static void
foreign_realloc(void) {
  void *pointer = malloc(7);
  assert(pointer);
  (void)xeme_audit_realloc(pointer, 11);
}

static void
interior_free(void) {
  char *pointer = xeme_audit_malloc(7);
  assert(pointer);
  xeme_audit_free(pointer + 1);
}

static void
check_child(void (*test)(void), int expected) {
  fflush(NULL);
  pid_t child = fork();
  assert(child >= 0);
  if (!child) {
    alarm(5);
    test();
    _Exit(0);
  }
  int status;
  assert(waitpid(child, &status, 0) == child);
  assert(WIFEXITED(status) && WEXITSTATUS(status) == expected);
}

int
main(void) {
  check_child(valid_ownership, 0);
  check_child(leak, 102);
  check_child(double_free, 102);
  check_child(foreign_free, 102);
  check_child(foreign_realloc, 102);
  check_child(interior_free, 102);
  puts("allocation tracker self-checks passed: 6 child scenarios");
  return 0;
}

/* Reproduces Expat 2.8.4's test allocator bug without linking any XML parser. */
#include <stddef.h>
#include "memcheck.h"

int main(void) {
  void *first = tracking_malloc(1);
  void *second = tracking_malloc(1);
  tracking_free(second);
  void *third = tracking_malloc(1);
  tracking_free(third);
  tracking_free(first);
  return !tracking_report();
}

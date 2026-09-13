/* Test-only bookkeeping. These direct libc calls are outside the upstream
 * injected-failure counters; only the diagnostic adapters call our wrappers. */
#include "allocation_tracker.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct Allocation {
  void *pointer;
  size_t size;
  struct Allocation *next;
} Allocation;

static Allocation *allocations;
static size_t malloc_calls, realloc_calls, free_calls;
static size_t injected_malloc_failures, injected_realloc_failures;
static size_t injected_failures;
static size_t system_failures, live_blocks, live_bytes;
static size_t peak_live_blocks, peak_live_bytes;

static void
audit_error(const char *message, const void *pointer) {
  fprintf(stderr, "XEME_ALLOCATION_ERROR\t%s\tpointer=%p\tlive_blocks=%zu\n",
          message, pointer, live_blocks);
  fflush(NULL);
  _Exit(102);
}

static void
increment(size_t *counter) {
  if (*counter == SIZE_MAX)
    audit_error("diagnostic counter overflow", NULL);
  ++*counter;
}

static Allocation **
find_allocation(void *pointer) {
  Allocation **entry = &allocations;
  while (*entry && (*entry)->pointer != pointer)
    entry = &(*entry)->next;
  if (!*entry)
    audit_error("pointer is not a live allocation", pointer);
  return entry;
}

static void
resize_accounting(size_t old_size, size_t new_size) {
  if (old_size > live_bytes || new_size > SIZE_MAX - (live_bytes - old_size))
    audit_error("diagnostic byte accounting overflow", NULL);
  live_bytes = live_bytes - old_size + new_size;
  if (live_bytes > peak_live_bytes)
    peak_live_bytes = live_bytes;
}

static void
remember(void *pointer, size_t size) {
  Allocation *entry = malloc(sizeof(*entry));
  if (!entry)
    audit_error("diagnostic metadata allocation failed", pointer);
  *entry = (Allocation){pointer, size, allocations};
  allocations = entry;
  increment(&live_blocks);
  if (live_blocks > peak_live_blocks)
    peak_live_blocks = live_blocks;
  resize_accounting(0, size);
}

static void
forget(Allocation **location) {
  Allocation *entry = *location;
  *location = entry->next;
  resize_accounting(entry->size, 0);
  --live_blocks;
  free(entry);
}

void *
xeme_audit_malloc(size_t size) {
  increment(&malloc_calls);
  void *pointer = malloc(size);
  if (pointer)
    remember(pointer, size);
  else if (size)
    increment(&system_failures);
  return pointer;
}

void *
xeme_audit_realloc(void *pointer, size_t size) {
  increment(&realloc_calls);
  Allocation **location = pointer ? find_allocation(pointer) : NULL;
  /* Use the free-and-NULL convention for zero-sized realloc, as the upstream
   * tracking allocator does. Injected failures are decided before this call. */
  if (pointer && !size) {
    forget(location);
    free(pointer);
    return NULL;
  }
  void *replacement = realloc(pointer, size);
  if (!replacement) {
    if (size)
      increment(&system_failures);
    /* A failed nonzero realloc leaves the original pointer and entry live. */
    return NULL;
  }
  if (location) {
    resize_accounting((*location)->size, size);
    (*location)->pointer = replacement;
    (*location)->size = size;
  } else {
    remember(replacement, size);
  }
  return replacement;
}

void
xeme_audit_free(void *pointer) {
  increment(&free_calls);
  if (!pointer)
    return;
  forget(find_allocation(pointer));
  free(pointer);
}

void
xeme_audit_failure(int is_realloc) {
  increment(&injected_failures);
  increment(is_realloc ? &injected_realloc_failures : &injected_malloc_failures);
}

size_t
xeme_audit_failure_count(void) {
  return injected_failures;
}

void
xeme_audit_checkpoint(void) {
  if (allocations || live_blocks || live_bytes)
    audit_error("allocations remain after teardown", allocations ? allocations->pointer : NULL);
}

void
xeme_audit_finish(void) {
  xeme_audit_checkpoint();
  printf("XEME_ALLOCATION_AUDIT\t{\"malloc_calls\":%zu,\"realloc_calls\":%zu,"
         "\"free_calls\":%zu,\"injected_malloc_failures\":%zu,"
         "\"injected_realloc_failures\":%zu,\"system_failures\":%zu,"
         "\"live_blocks\":%zu,\"peak_live_blocks\":%zu,\"peak_live_bytes\":%zu}\n",
         malloc_calls, realloc_calls, free_calls, injected_malloc_failures,
         injected_realloc_failures, system_failures, live_blocks, peak_live_blocks,
         peak_live_bytes);
  fflush(stdout);
}

/* Diagnostic ownership tracking for Expat's injected-failure allocators. */
#ifndef XEME_ALLOCATION_TRACKER_H
#define XEME_ALLOCATION_TRACKER_H

#include <stddef.h>

void *xeme_audit_malloc(size_t size);
void *xeme_audit_realloc(void *pointer, size_t size);
void xeme_audit_free(void *pointer);
void xeme_audit_failure(int is_realloc);
size_t xeme_audit_failure_count(void);
void xeme_audit_checkpoint(void);
void xeme_audit_finish(void);

#endif

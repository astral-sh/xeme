/* Exhaustive failing-allocation and allocator ownership gate for the C ABI. */
#include "expat.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    void *pointer;
    size_t size;
} Block;
typedef struct {
    size_t calls, fail_at, live, allocated, freed;
    int failed;
    Block blocks[8192];
} Tracker;

static Tracker first, second;

static int should_fail(Tracker *tracker) {
    tracker->calls++;
    if (tracker->fail_at && tracker->calls >= tracker->fail_at) {
        tracker->failed = 1;
        return 1;
    }
    return 0;
}

static Block *find(Tracker *tracker, void *pointer) {
    for (size_t index = 0; index < sizeof(tracker->blocks) / sizeof(Block); index++) {
        if (tracker->blocks[index].pointer == pointer) return &tracker->blocks[index];
    }
    assert(!"allocation freed through the wrong suite, twice, or with an interior pointer");
    return NULL;
}

static void remember(Tracker *tracker, void *pointer, size_t size) {
    assert(pointer);
    for (size_t index = 0; index < sizeof(tracker->blocks) / sizeof(Block); index++) {
        if (!tracker->blocks[index].pointer) {
            tracker->blocks[index] = (Block){pointer, size};
            tracker->live++;
            tracker->allocated++;
            return;
        }
    }
    assert(!"test allocation table is too small");
}

static void *tracked_malloc(Tracker *tracker, size_t size) {
    if (should_fail(tracker)) return NULL;
    void *pointer = malloc(size ? size : 1);
    remember(tracker, pointer, size);
    return pointer;
}

static void tracked_free(Tracker *tracker, void *pointer) {
    if (!pointer) return;
    Block *block = find(tracker, pointer);
    /* Poison before release: no parser-owned reference may outlive this call. */
    memset(pointer, 0xA5, block->size);
    free(pointer);
    block->pointer = NULL;
    tracker->live--;
    tracker->freed++;
}

static void *tracked_realloc(Tracker *tracker, void *pointer, size_t size) {
    if (should_fail(tracker)) return NULL;
    if (!pointer) {
        void *replacement = malloc(size ? size : 1);
        remember(tracker, replacement, size);
        return replacement;
    }
    Block *block = find(tracker, pointer);
    void *replacement = realloc(pointer, size ? size : 1);
    assert(replacement);
    block->pointer = replacement;
    block->size = size;
    return replacement;
}

static void *malloc_first(size_t size) { return tracked_malloc(&first, size); }
static void *realloc_first(void *p, size_t size) { return tracked_realloc(&first, p, size); }
static void free_first(void *p) { tracked_free(&first, p); }
static void *malloc_second(size_t size) { return tracked_malloc(&second, size); }
static void *realloc_second(void *p, size_t size) { return tracked_realloc(&second, p, size); }
static void free_second(void *p) { tracked_free(&second, p); }

static const XML_Memory_Handling_Suite suite_first = {malloc_first, realloc_first, free_first};
static const XML_Memory_Handling_Suite suite_second = {malloc_second, realloc_second, free_second};

typedef struct {
    XML_Parser parser;
    size_t elements, models;
    int nested;
} Context;

static void start(void *data, const XML_Char *name, const XML_Char **attributes) {
    Context *context = data;
    context->elements++;
    assert(name && attributes);
    if (!context->nested) return;
    XML_Parser other = XML_ParserCreate_MM(NULL, &suite_second, NULL);
    if (!other) {
        assert(second.failed);
        assert(XML_StopParser(context->parser, XML_FALSE) == XML_STATUS_OK);
        return;
    }
    enum XML_Status status = XML_Parse(other, "<other/>", 8, XML_TRUE);
    assert(status == XML_STATUS_OK || second.failed);
    XML_ParserFree(other);
    if (status != XML_STATUS_OK) {
        assert(XML_StopParser(context->parser, XML_FALSE) == XML_STATUS_OK);
    }
}

static void model(void *data, const XML_Char *name, XML_Content *content) {
    Context *context = data;
    assert(name && content);
    context->models++;
    XML_FreeContentModel(context->parser, content);
}

static void check_freed(void) {
    assert(first.live == 0 && first.allocated == first.freed);
    assert(second.live == 0 && second.allocated == second.freed);
}

static int document_case(size_t fail_at, int route, int fail_second) {
    static const char xml[] =
        "<!DOCTYPE r [<!ELEMENT r (#PCDATA|n)*><!ELEMENT n EMPTY>"
        "<!ENTITY e 'text'><!ATTLIST r a CDATA 'value'>]>"
        "<r xmlns:p='urn:p' p:key='value'>&e;<n/></r>";
    memset(&first, 0, sizeof(first));
    memset(&second, 0, sizeof(second));
    (fail_second ? &second : &first)->fail_at = fail_at;
    XML_Parser parser = XML_ParserCreate_MM(NULL, &suite_first, "|");
    if (!parser) {
        assert(first.failed);
        check_freed();
        return 1;
    }
    Context context = {parser, 0, 0, route == 2};
    XML_SetUserData(parser, &context);
    XML_SetElementHandler(parser, start, NULL);
    XML_SetElementDeclHandler(parser, model);
    enum XML_Status status = XML_SetBase(parser, "urn:base");
    if (status == XML_STATUS_OK) {
        assert(strcmp(XML_GetBase(parser), "urn:base") == 0);
        if (route == 1) {
            void *buffer = XML_GetBuffer(parser, (int)sizeof(xml) - 1);
            if (buffer) {
                memcpy(buffer, xml, sizeof(xml) - 1);
                status = XML_ParseBuffer(parser, (int)sizeof(xml) - 1, XML_TRUE);
            } else status = XML_STATUS_ERROR;
        } else {
            status = XML_Parse(parser, xml, (int)sizeof(xml) - 1, XML_TRUE);
        }
    }
    int failed = first.failed || second.failed;
    if (failed) {
        assert(status == XML_STATUS_ERROR);
    } else {
        assert(status == XML_STATUS_OK && context.elements == 2 && context.models == 2);
        /* Reset also frees allocator-owned namespaces, DTDs, and handler state. */
        if (XML_ParserReset(parser, NULL)) {
            status = XML_Parse(parser, "<again/>", 8, XML_TRUE);
            assert(status == XML_STATUS_OK || first.failed);
        } else assert(first.failed);
    }
    failed = first.failed || second.failed;
    XML_ParserFree(parser);
    check_freed();
    return failed;
}

static void memory_helpers(void) {
    memset(&first, 0, sizeof(first));
    XML_Parser parser = XML_ParserCreate_MM(NULL, &suite_first, NULL);
    assert(parser);
    unsigned char *block = XML_MemMalloc(parser, 31);
    assert(block);
    memset(block, 0x3C, 31);
    first.fail_at = first.calls + 1;
    assert(XML_MemRealloc(parser, block, 65536) == NULL);
    for (size_t index = 0; index < 31; index++) assert(block[index] == 0x3C);
    first.fail_at = 0;
    block = XML_MemRealloc(parser, block, 100);
    assert(block);
    for (size_t index = 0; index < 31; index++) assert(block[index] == 0x3C);
    XML_MemFree(parser, block);
    XML_ParserFree(parser);
    check_freed();
}

int main(void) {
    size_t scenarios = 0;
    for (int route = 0; route < 3; route++) {
        for (int fail_second = 0; fail_second <= (route == 2); fail_second++) {
            int finished = 0;
            for (size_t fail_at = 1; fail_at < 10000; fail_at++) {
                scenarios++;
                if (!document_case(fail_at, route, fail_second)) {
                    finished = 1;
                    break;
                }
            }
            assert(finished);
        }
    }
    memory_helpers();
    printf("allocation failure sweep passed: %zu scenarios (%s)\n", scenarios, XML_ExpatVersion());
    return 0;
}

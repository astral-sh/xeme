/* Public callback guards, plus Oriole's bounded recursive-default rejection. */
#define XML_GE 1
#include "expat.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static XML_Parser parser;
static int calls;
static int operation;

static void XMLCALL on_default(void *data, const char *text, int length) {
    assert(data == &calls);
    assert(text && length > 0);
    calls++;
    XML_DefaultCurrent(parser); /* Bounded rejection of recursive default dispatch. */
}

static void XMLCALL on_start(void *data, const char *name, const char **attributes) {
    assert(data == &calls);
    calls++;
    int offset = -1;
    int length = -1;
    const char *context = XML_GetInputContext(parser, &offset, &length);
    assert(context && offset >= 0 && length - offset >= 14);
    assert(memcmp(context + offset, "<r a='value'/>", 14) == 0);
    switch (operation) {
    case 0:
        XML_ParserFree(parser);
        /* Callback-time Free is ignored; the caller still owns the handle. */
        assert(strcmp(name, "r") == 0);
        assert(strcmp(attributes[0], "a") == 0);
        assert(strcmp(attributes[1], "value") == 0);
        break;
    case 1:
        assert(XML_Parse(parser, "<x/>", 4, XML_TRUE) == XML_STATUS_ERROR);
        break;
    case 2:
        assert(XML_ParserReset(parser, NULL) == XML_FALSE);
        break;
    case 3:
        assert(XML_GetBuffer(parser, 16) == NULL);
        break;
    case 4:
        XML_DefaultCurrent(parser);
        break;
    case 5: {
        XML_Parser child = XML_ParserCreate(NULL);
        assert(child);
        assert(XML_Parse(child, "<independent/>", 14, XML_TRUE) == XML_STATUS_OK);
        XML_ParserFree(child);
        assert(XML_SetBase(parser, XML_GetBase(parser)) == XML_STATUS_OK);
        assert(strcmp(XML_GetBase(parser), "urn:base") == 0);
        assert(XML_SetAllocTrackerActivationThreshold(parser, 0) == XML_TRUE);
        void *memory = XML_MemMalloc(parser, 1000);
        assert(memory);
        memset(memory, 0x55, 1000);
        memory = XML_MemRealloc(parser, memory, 2000);
        assert(memory);
        memset(memory, 0xaa, 2000);
        XML_MemFree(parser, memory);
        assert(XML_SetAllocTrackerActivationThreshold(parser, 64ULL * 1024 * 1024)
               == XML_TRUE);
        break;
    }
    default:
        assert(0);
    }
    /* Callback-safe mutations and nested parsing must not invalidate this view. */
    assert(memcmp(context + offset, "<r a='value'/>", 14) == 0);
}

int main(void) {
    for (operation = 0; operation < 6; operation++) {
        parser = XML_ParserCreate(NULL);
        assert(parser);
        calls = 0;
        XML_SetUserData(parser, &calls);
        XML_SetStartElementHandler(parser, on_start);
        assert(XML_SetBase(parser, "urn:base") == XML_STATUS_OK);
        if (operation == 4) XML_SetDefaultHandler(parser, on_default);
        int status = XML_Parse(parser, "<r a='value'/>", 14, XML_TRUE);
        assert(calls == (operation == 4 ? 2 : 1));
        if (operation == 4) {
            assert(status == XML_STATUS_ERROR);
        } else {
            assert(status == XML_STATUS_OK);
            assert(XML_GetErrorCode(parser) == XML_ERROR_NONE);
        }
        XML_ParserFree(parser);
    }
    puts("Oriole native adversarial lifecycle probes passed");
    return 0;
}

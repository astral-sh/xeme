/* Public callback guards, plus Oriole's bounded recursive-default rejection. */
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
        break;
    }
    default:
        assert(0);
    }
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

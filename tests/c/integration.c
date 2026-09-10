/* Public-header consumer assertions. Run against both Expat and Oriole. */
#include "expat.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int starts, ends;
static char text_output[128];
static size_t text_length;
static XML_Parser active;
static int suspend_once;

static void XMLCALL on_start(void *data, const char *name, const char **attributes) {
    assert(data == &starts);
    assert(name && attributes);
    starts++;
    if (suspend_once) {
        suspend_once = 0;
        assert(XML_StopParser(active, XML_TRUE) == XML_STATUS_OK);
    }
}
static void XMLCALL on_end(void *data, const char *name) {
    assert(data == &starts);
    assert(name);
    ends++;
}
static void XMLCALL on_text(void *data, const char *text, int length) {
    assert(data == &starts);
    assert(length >= 0 && text_length + (size_t)length < sizeof(text_output));
    memcpy(text_output + text_length, text, (size_t)length);
    text_length += (size_t)length;
    text_output[text_length] = 0;
}
static XML_Parser configured(void) {
    XML_Parser parser = XML_ParserCreate(NULL);
    assert(parser);
    starts = ends = 0;
    text_length = 0;
    text_output[0] = 0;
    XML_SetUserData(parser, &starts);
    assert(XML_GetUserData(parser) == &starts); /* Public macro checks first-field ABI. */
    XML_SetElementHandler(parser, on_start, on_end);
    XML_SetCharacterDataHandler(parser, on_text);
    return parser;
}
static void incremental(void) {
    const char *input = "<root><child a=\"v\">one &amp; two</child></root>";
    XML_Parser parser = configured();
    for (size_t i = 0; i < strlen(input); i++) {
        assert(XML_Parse(parser, input + i, 1, XML_FALSE) == XML_STATUS_OK);
    }
    assert(XML_Parse(parser, NULL, 0, XML_TRUE) == XML_STATUS_OK);
    assert(starts == 2 && ends == 2);
    assert(strcmp(text_output, "one & two") == 0);
    assert(XML_GetErrorCode(parser) == XML_ERROR_NONE);
    assert(XML_Parse(parser, "", 0, XML_TRUE) == XML_STATUS_ERROR);
    assert(XML_GetErrorCode(parser) == XML_ERROR_FINISHED);
    assert(XML_ParserReset(parser, NULL));
    assert(XML_GetUserData(parser) == NULL);
    assert(XML_Parse(parser, "<again/>", 8, XML_TRUE) == XML_STATUS_OK);
    /* Reset clears handlers. */
    assert(starts == 2 && ends == 2);
    XML_ParserFree(parser);
}
static void buffer_api(void) {
    XML_Parser parser = configured();
    void *buffer = XML_GetBuffer(parser, 32);
    assert(buffer);
    memcpy(buffer, "<r>buffer</r>", 13);
    assert(XML_ParseBuffer(parser, 13, XML_TRUE) == XML_STATUS_OK);
    assert(starts == 1 && ends == 1);
    assert(strcmp(text_output, "buffer") == 0);
    XML_ParserFree(parser);
}
static void suspend_resume(void) {
    active = configured();
    suspend_once = 1;
    assert(XML_Parse(active, "<r><c/></r>", 11, XML_TRUE) == XML_STATUS_SUSPENDED);
    XML_ParsingStatus status;
    XML_GetParsingStatus(active, &status);
    assert(status.parsing == XML_SUSPENDED);
    assert(XML_ResumeParser(active) == XML_STATUS_OK);
    assert(starts == 2 && ends == 2);
    XML_GetParsingStatus(active, &status);
    assert(status.parsing == XML_FINISHED);
    XML_ParserFree(active);
    active = NULL;
}
static size_t allocations, frees;
static void *custom_malloc(size_t size) {
    void *result = malloc(size);
    if (result) allocations++;
    return result;
}
static void *custom_realloc(void *pointer, size_t size) {
    if (!pointer) return custom_malloc(size);
    return realloc(pointer, size);
}
static void custom_free(void *pointer) {
    if (pointer) frees++;
    free(pointer);
}
static void custom_memory(void) {
    XML_Memory_Handling_Suite memory = {custom_malloc, custom_realloc, custom_free};
    XML_Parser parser = XML_ParserCreate_MM(NULL, &memory, NULL);
    assert(parser);
    void *block = XML_MemMalloc(parser, 20);
    assert(block);
    block = XML_MemRealloc(parser, block, 40);
    assert(block);
    XML_MemFree(parser, block);
    assert(XML_Parse(parser, "<r/>", 4, XML_TRUE) == XML_STATUS_OK);
    XML_ParserFree(parser);
    assert(allocations > 0 && allocations == frees);
}
static void *fail_malloc(size_t size) { (void)size; return NULL; }
static void *fail_realloc(void *pointer, size_t size) { (void)pointer; (void)size; return NULL; }
static void fail_free(void *pointer) { assert(!pointer); }
static void failed_allocation(void) {
    XML_Memory_Handling_Suite memory = {fail_malloc, fail_realloc, fail_free};
    assert(XML_ParserCreate_MM(NULL, &memory, NULL) == NULL);
}
static void parser_as_handler(void *context, const char *name, const char **attributes) {
    assert(context == active);
    assert(XML_GetUserData((XML_Parser)context) == &starts);
    assert(strcmp(name, "r") == 0 && !attributes[0]);
    starts++;
}
static void handler_argument(void) {
    active = configured();
    XML_SetElementHandler(active, parser_as_handler, NULL);
    XML_UseParserAsHandlerArg(active);
    assert(XML_Parse(active, "<r/>", 4, XML_TRUE) == XML_STATUS_OK);
    assert(starts == 1);
    XML_ParserFree(active);
    active = NULL;
}
int main(int argc, char **argv) {
    if (argc == 2 && strcmp(argv[1], "--unsupported-mm-contract") == 0) {
        XML_Memory_Handling_Suite memory = {custom_malloc, custom_realloc, custom_free};
        assert(XML_ParserCreate_MM(NULL, &memory, NULL) == NULL);
        puts("Custom memory suite explicitly rejected; this is an unsupported-feature contract, not Expat compatibility");
        return 0;
    }
    int supported = argc == 2 && strcmp(argv[1], "--supported") == 0;
    if (argc != 1 && !supported) {
        fprintf(stderr, "usage: %s [--supported|--unsupported-mm-contract]\n", argv[0]);
        return 2;
    }
    incremental();
    buffer_api();
    suspend_resume();
    handler_argument();
    if (!supported) {
        custom_memory();
        failed_allocation();
    }
    printf("C ABI %s integration passed (%s)\n", supported ? "supported-subset" : "full", XML_ExpatVersion());
    return 0;
}

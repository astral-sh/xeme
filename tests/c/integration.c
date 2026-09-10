/* Public-header consumer assertions. Run against both Expat and Oriole. */
#define XML_DTD 1
#define XML_GE 1
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

static void XMLCALL restrict_expansion(void *data, const char *name,
                                     const char **attributes) {
    (void)name;
    (void)attributes;
    XML_Parser parser = data;
    assert(XML_SetBillionLaughsAttackProtectionMaximumAmplification(parser, 1.0f));
    assert(XML_SetBillionLaughsAttackProtectionActivationThreshold(parser, 0));
}

static void entity_amplification(void) {
    const char *prefix = "<!DOCTYPE r [<!ENTITY e 'abcdefghijklmnop'>]><r>&e;</r>";
    char input[4096];
    memset(input, ' ', sizeof(input));
    memcpy(input, prefix, strlen(prefix));
    for (int buffered = 0; buffered < 2; buffered++) {
        XML_Parser parser = XML_ParserCreate(NULL);
        assert(parser);
        XML_UseParserAsHandlerArg(parser);
        XML_SetStartElementHandler(parser, restrict_expansion);
        enum XML_Status status;
        if (buffered) {
            void *buffer = XML_GetBuffer(parser, sizeof(input));
            assert(buffer);
            memcpy(buffer, input, sizeof(input));
            status = XML_ParseBuffer(parser, sizeof(input), XML_TRUE);
        } else {
            status = XML_Parse(parser, input, sizeof(input), XML_TRUE);
        }
        /* A callback can tighten the limit; trailing unparsed bytes cannot
           dilute the root's already consumed input. */
        assert(status == XML_STATUS_ERROR);
        assert(XML_GetErrorCode(parser) == XML_ERROR_AMPLIFICATION_LIMIT_BREACH);
        assert(XML_ParserReset(parser, NULL));
        assert(XML_Parse(parser, prefix, (int)strlen(prefix), XML_TRUE) == XML_STATUS_OK);
        XML_ParserFree(parser);
    }
}

int main(void) {
    incremental();
    buffer_api();
    suspend_resume();
    handler_argument();
    custom_memory();
    failed_allocation();
    entity_amplification();
    printf("C ABI full integration passed (%s)\n", XML_ExpatVersion());
    return 0;
}

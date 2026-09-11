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

static void version_identity(void) {
    XML_Expat_Version version = XML_ExpatVersionInfo();
    const char *label = XML_ExpatVersion();
    const char *digits = label + strcspn(label, "0123456789");
    int major, minor, micro, consumed = 0;
    assert(sscanf(digits, "%d.%d.%d%n", &major, &minor, &micro, &consumed) == 3);
    assert(digits[consumed] == '\0');
    assert(major == version.major && minor == version.minor && micro == version.micro);
}

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
/* Successful namespace behavior from Expat's test_nsalloc_long_element.
 * See UPSTREAM-NOTICES.txt for its license. Allocation-failure schedules remain
 * covered by the separate upstream gate. */
static const char namespace_long_element[] =
    "http://example.org/ thisisalongenoughelementnametotriggerareallocation foo";
static const char namespace_long_attribute[] = "http://example.org/ a bar";

static void XMLCALL namespace_long_start(void *data, const char *name,
                                        const char **attributes) {
    int *calls = data;
    assert(strcmp(name, namespace_long_element) == 0);
    assert(attributes && attributes[0]);
    assert(strcmp(attributes[0], namespace_long_attribute) == 0);
    assert(attributes[1] && strcmp(attributes[1], "12") == 0 && !attributes[2]);
    calls[0]++;
}

static void XMLCALL namespace_long_end(void *data, const char *name) {
    int *calls = data;
    assert(strcmp(name, namespace_long_element) == 0);
    calls[1]++;
}

static void namespace_long_names(void) {
    const char *input =
        "<foo:thisisalongenoughelementnametotriggerareallocation\n"
        " xmlns:foo='http://example.org/' bar:a='12'\n"
        " xmlns:bar='http://example.org/'>"
        "</foo:thisisalongenoughelementnametotriggerareallocation>";
    const XML_Char separator[] = " ";
    XML_Memory_Handling_Suite memory = {custom_malloc, custom_realloc, custom_free};
#if XML_MAJOR_VERSION > 2 || (XML_MAJOR_VERSION == 2 && XML_MINOR_VERSION >= 6)
    const int deferral_modes = 2;
#else
    const int deferral_modes = 1;
#endif
    for (int chunk = 0; chunk <= 5; chunk++) {
        for (int deferral = 0; deferral < deferral_modes; deferral++) {
            int calls[2] = {0, 0};
            XML_Parser parser = XML_ParserCreate_MM(NULL, &memory, separator);
            assert(parser);
#if XML_MAJOR_VERSION > 2 || (XML_MAJOR_VERSION == 2 && XML_MINOR_VERSION >= 6)
            assert(XML_SetReparseDeferralEnabled(parser, (XML_Bool)deferral));
#endif
            XML_SetReturnNSTriplet(parser, XML_TRUE);
            XML_SetUserData(parser, calls);
            XML_SetElementHandler(parser, namespace_long_start, namespace_long_end);
            const char *next = input;
            int remaining = (int)strlen(input);
            if (chunk > 0) {
                while (remaining > chunk) {
                    assert(XML_Parse(parser, next, chunk, XML_FALSE) == XML_STATUS_OK);
                    next += chunk;
                    remaining -= chunk;
                }
            }
            assert(XML_Parse(parser, next, remaining, XML_TRUE) == XML_STATUS_OK);
            assert(XML_GetErrorCode(parser) == XML_ERROR_NONE);
            assert(calls[0] == 1 && calls[1] == 1);
            XML_ParserFree(parser);
            assert(allocations == frees);
        }
    }
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

static int encoding_releases;
static int XMLCALL alias_convert(void *data, const char *input) {
    assert(data == &encoding_releases);
    return (unsigned char)input[1];
}
static void XMLCALL alias_release(void *data) {
    assert(data == &encoding_releases);
    encoding_releases++;
}
static int XMLCALL alias_encoding(void *data, const char *name, XML_Encoding *info) {
    assert(data == &encoding_releases && strcmp(name, "alias") == 0);
    for (int i = 0; i < 256; i++) info->map[i] = i < 128 ? i : -1;
    info->map[128] = info->map[129] = -2;
    info->data = data;
    info->convert = alias_convert;
    info->release = alias_release;
    return XML_STATUS_OK;
}
static void XMLCALL alias_start(void *data, const char *name, const char **attributes) {
    on_start(data, name, attributes);
    if (attributes[0]) {
        assert(strcmp(attributes[0], "a") == 0);
        assert(strcmp(attributes[1], "<\r\n") == 0);
        assert(!attributes[2]);
    }
}
static void custom_encoding_aliases(void) {
    const struct {
        const char *input;
        enum XML_Error error;
        const char *text;
    } cases[] = {
        {"<r a='\200<\200\r\200\n'>\200&\200<\200\r\200\n</r>", XML_ERROR_NONE, "&<\r\n"},
        {"<\200A></\200A>", XML_ERROR_NONE, ""},
        {"<\200A></\201A>", XML_ERROR_TAG_MISMATCH, ""},
        {"<r \200A='x' A='y'/>", XML_ERROR_DUPLICATE_ATTRIBUTE, ""},
    };
    for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
        for (int buffered = 0; buffered < 2; buffered++) {
            for (int incremental_input = 0; incremental_input < 2; incremental_input++) {
                XML_Parser parser = configured();
                assert(XML_SetEncoding(parser, "alias"));
                XML_SetStartElementHandler(parser, alias_start);
                encoding_releases = 0;
                XML_SetUnknownEncodingHandler(parser, alias_encoding, &encoding_releases);
                size_t length = strlen(cases[i].input);
                enum XML_Status status = XML_STATUS_OK;
                for (size_t offset = 0; offset < length && status == XML_STATUS_OK;) {
                    int count = incremental_input ? 1 : (int)length;
                    int final = offset + (size_t)count == length;
                    if (buffered) {
                        void *buffer = XML_GetBuffer(parser, count);
                        assert(buffer);
                        memcpy(buffer, cases[i].input + offset, (size_t)count);
                        status = XML_ParseBuffer(parser, count, final);
                    } else {
                        status = XML_Parse(parser, cases[i].input + offset, count, final);
                    }
                    offset += (size_t)count;
                }
                assert(status == (cases[i].error == XML_ERROR_NONE ? XML_STATUS_OK : XML_STATUS_ERROR));
                assert(XML_GetErrorCode(parser) == cases[i].error);
                assert(strcmp(text_output, cases[i].text) == 0);
                assert(encoding_releases == 0);
                XML_ParserFree(parser);
                assert(encoding_releases == 1);
            }
        }
    }
}

int main(void) {
    version_identity();
    incremental();
    buffer_api();
    suspend_resume();
    handler_argument();
    custom_memory();
    namespace_long_names();
    failed_allocation();
    entity_amplification();
    custom_encoding_aliases();
    printf("C ABI full integration passed (%s)\n", XML_ExpatVersion());
    return 0;
}

#define _GNU_SOURCE
#include <dlfcn.h>
#include <limits.h>
#define main integration_main
#include "integration.c"
#undef main

static void check_origin(void *symbol, const char *expected) {
    Dl_info info;
    char actual[PATH_MAX];
    assert(dladdr(symbol, &info) && info.dli_fname);
    assert(realpath(info.dli_fname, actual));
    assert(strcmp(actual, expected) == 0);
}

int main(int argc, char **argv) {
    char expected[PATH_MAX];
    assert(argc == 2 && realpath(argv[1], expected));
    check_origin((void *)XML_ExpatVersion, expected);
    check_origin((void *)XML_ExpatVersionInfo, expected);
    check_origin((void *)XML_ExternalEntityParserCreate, expected);
    check_origin((void *)XML_GetBuffer, expected);
    check_origin((void *)XML_GetCurrentByteIndex, expected);
    check_origin((void *)XML_GetCurrentColumnNumber, expected);
    check_origin((void *)XML_GetCurrentLineNumber, expected);
    check_origin((void *)XML_GetErrorCode, expected);
    check_origin((void *)XML_GetParsingStatus, expected);
    check_origin((void *)XML_MemFree, expected);
    check_origin((void *)XML_MemMalloc, expected);
    check_origin((void *)XML_MemRealloc, expected);
    check_origin((void *)XML_Parse, expected);
    check_origin((void *)XML_ParseBuffer, expected);
    check_origin((void *)XML_ParserCreate, expected);
    check_origin((void *)XML_ParserCreate_MM, expected);
    check_origin((void *)XML_ParserFree, expected);
    check_origin((void *)XML_ParserReset, expected);
    check_origin((void *)XML_ResumeParser, expected);
    check_origin((void *)XML_SetBillionLaughsAttackProtectionActivationThreshold, expected);
    check_origin((void *)XML_SetBillionLaughsAttackProtectionMaximumAmplification, expected);
    check_origin((void *)XML_SetCharacterDataHandler, expected);
    check_origin((void *)XML_SetElementHandler, expected);
    check_origin((void *)XML_SetEncoding, expected);
    check_origin((void *)XML_SetEntityDeclHandler, expected);
    check_origin((void *)XML_SetExternalEntityRefHandler, expected);
    check_origin((void *)XML_SetParamEntityParsing, expected);
    check_origin((void *)XML_SetReparseDeferralEnabled, expected);
    check_origin((void *)XML_SetReturnNSTriplet, expected);
    check_origin((void *)XML_SetStartElementHandler, expected);
    check_origin((void *)XML_SetUnknownEncodingHandler, expected);
    check_origin((void *)XML_SetUserData, expected);
    check_origin((void *)XML_StopParser, expected);
    check_origin((void *)XML_UseParserAsHandlerArg, expected);
    printf("Verified 34 API symbol origins: %s\n", expected);
    fflush(stdout);
    int result = integration_main();
    printf("Phase-relative OOM passed: successful_declarations=2 child_error=%d parent_error=%d rejected_allocations=%zu live_allocations=%zu\n",
           XML_ERROR_NO_MEMORY, XML_ERROR_EXTERNAL_ENTITY_HANDLING,
           nested_oom_failures, allocations - frees);
    return result;
}

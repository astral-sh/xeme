#define _POSIX_C_SOURCE 200809L
#include <dlfcn.h>
#include <errno.h>
#include <inttypes.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include "expat.h"

typedef struct {
    XML_Parser (*create)(const XML_Char *);
    XML_Parser (*child)(XML_Parser, const XML_Char *, const XML_Char *);
    void (*destroy)(XML_Parser);
    enum XML_Status (*parse)(XML_Parser, const char *, int, int);
    enum XML_Error (*error)(XML_Parser);
    void (*user_data)(XML_Parser, void *);
    void (*external)(XML_Parser, XML_ExternalEntityRefHandler);
    int (*parameters)(XML_Parser, enum XML_ParamEntityParsing);
    void (*elements)(XML_Parser, XML_StartElementHandler, XML_EndElementHandler);
    void (*entity)(XML_Parser, XML_EntityDeclHandler);
    void (*attlist)(XML_Parser, XML_AttlistDeclHandler);
    void (*model)(XML_Parser, XML_ElementDeclHandler);
    void (*free_model)(XML_Parser, XML_Content *);
    const XML_LChar *(*version)(void);
} Api;

typedef struct State {
    Api *api;
    const char *data;
    size_t length, chunk;
    uint64_t hash;
    size_t declarations, external_calls;
    XML_Parser active;
    unsigned depth;
    int trace;
} State;

static void bytes(State *s, const void *data, size_t len) {
    const unsigned char *p = data;
    if (s->trace) for (size_t i = 0; i < len; i++) printf("%02x", p[i]);
    for (size_t i = 0; i < len; i++) {
        s->hash ^= p[i];
        s->hash *= UINT64_C(1099511628211);
    }
}
static void field(State *s, const char *text) {
    unsigned char present = text != NULL;
    bytes(s, &present, 1);
    if (text) bytes(s, text, strlen(text));
    bytes(s, "\0", 1);
}
static void number(State *s, unsigned value) {
    unsigned char encoded[4];
    for (unsigned i = 0; i < 4; i++) encoded[i] = (unsigned char)(value >> (i * 8));
    bytes(s, encoded, 4);
}
static void XMLCALL start(void *data, const XML_Char *name, const XML_Char **attrs) {
    State *s = data;
    field(s, "start"); number(s, s->depth); field(s, name);
    for (size_t i = 0; attrs[i]; i += 2) { field(s, attrs[i]); field(s, attrs[i + 1]); }
}
static void XMLCALL end(void *data, const XML_Char *name) {
    State *s = data;
    field(s, "end"); number(s, s->depth); field(s, name);
}
static void XMLCALL entity(void *data, const XML_Char *name, int parameter,
                          const XML_Char *value, int length, const XML_Char *base,
                          const XML_Char *system, const XML_Char *public_id,
                          const XML_Char *notation) {
    State *s = data;
    field(s, "entity"); number(s, s->depth); field(s, name); number(s, (unsigned)parameter);
    unsigned char present = value != NULL;
    bytes(s, &present, 1);
    if (value) bytes(s, value, (size_t)length);
    bytes(s, "\0", 1);
    field(s, base); field(s, system); field(s, public_id); field(s, notation);
    s->declarations++;
}
static void XMLCALL attlist(void *data, const XML_Char *element, const XML_Char *attribute,
                           const XML_Char *type, const XML_Char *value, int required) {
    State *s = data;
    field(s, "attribute"); number(s, s->depth); field(s, element); field(s, attribute);
    field(s, type); field(s, value); number(s, (unsigned)required);
    s->declarations++;
}
static void model_hash(State *s, const XML_Content *model) {
    number(s, (unsigned)model->type); number(s, (unsigned)model->quant);
    field(s, model->name); number(s, model->numchildren);
    for (unsigned i = 0; i < model->numchildren; i++) model_hash(s, &model->children[i]);
}
static void XMLCALL model(void *data, const XML_Char *name, XML_Content *content) {
    State *s = data;
    field(s, "element"); number(s, s->depth); field(s, name); model_hash(s, content);
    s->api->free_model(s->active, content);
    s->declarations++;
}
static int feed(State *s, XML_Parser parser, const char *data, size_t length) {
    size_t offset = 0;
    do {
        size_t count = length - offset;
        if (count > s->chunk) count = s->chunk;
        if (s->api->parse(parser, data + offset, (int)count, offset + count == length) != XML_STATUS_OK) {
            fprintf(stderr, "parse error %d at input chunk %zu\n", s->api->error(parser), offset);
            return 0;
        }
        offset += count;
    } while (offset < length);
    return 1;
}
static int XMLCALL external(XML_Parser parent, const XML_Char *context, const XML_Char *base,
                            const XML_Char *system, const XML_Char *public_id) {
    /* The fixture resolver has two in-memory resources and performs no I/O. */
    State *s = XML_GetUserData(parent);
    if (!system || s->depth >= 2 || (strcmp(system, "d") && strcmp(system, "empty"))) return 0;
    field(s, "external"); number(s, s->depth);
    field(s, context); field(s, base); field(s, system); field(s, public_id);
    XML_Parser child = s->api->child(parent, context, NULL);
    if (!child) return 0;
    XML_Parser previous = s->active;
    s->active = child; s->depth++; s->external_calls++;
    int result = feed(s, child, !strcmp(system, "d") ? s->data : "", !strcmp(system, "d") ? s->length : 0);
    s->api->destroy(child);
    s->active = previous; s->depth--;
    return result;
}
static double now(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value)) abort();
    return (double)value.tv_sec + (double)value.tv_nsec / 1e9;
}
static size_t positive(const char *value) {
    char *end;
    errno = 0;
    unsigned long long parsed = strtoull(value, &end, 10);
    if (errno || end == value || *end || *value == '-' || !parsed || parsed > INT_MAX) return 0;
    return (size_t)parsed;
}
#define LOAD(member, symbol) do { \
    void *address = dlsym(library, symbol); \
    if (!address || sizeof(api.member) != sizeof(address)) { fprintf(stderr, "missing %s\n", symbol); return 2; } \
    memcpy(&api.member, &address, sizeof(address)); \
} while (0)

int main(int argc, char **argv) {
    if (argc != 5 && (argc != 6 || strcmp(argv[5], "--trace"))) { fprintf(stderr, "usage: DRIVER LIBRARY DTD_FILE CHUNK ITERATIONS [--trace]\n"); return 2; }
    size_t chunk = positive(argv[3]), iterations = positive(argv[4]);
    if (!chunk || !iterations) return 2;
    FILE *file = fopen(argv[2], "rb");
    if (!file || fseek(file, 0, SEEK_END)) return 2;
    long length = ftell(file);
    if (length < 0 || length > 16 * 1024 * 1024 || fseek(file, 0, SEEK_SET)) return 2;
    char *data = malloc((size_t)length + 1);
    if (!data || fread(data, 1, (size_t)length, file) != (size_t)length) return 2;
    fclose(file);
    void *library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); return 2; }
    Api api;
    LOAD(create, "XML_ParserCreate"); LOAD(child, "XML_ExternalEntityParserCreate");
    LOAD(destroy, "XML_ParserFree"); LOAD(parse, "XML_Parse"); LOAD(error, "XML_GetErrorCode");
    LOAD(user_data, "XML_SetUserData"); LOAD(external, "XML_SetExternalEntityRefHandler");
    LOAD(parameters, "XML_SetParamEntityParsing"); LOAD(elements, "XML_SetElementHandler");
    LOAD(entity, "XML_SetEntityDeclHandler"); LOAD(attlist, "XML_SetAttlistDeclHandler");
    LOAD(model, "XML_SetElementDeclHandler"); LOAD(free_model, "XML_FreeContentModel");
    LOAD(version, "XML_ExpatVersion");
    printf("{\"version\":\"%s\",\"samples\":[", api.version());
    uint64_t expected = 0;
    for (size_t i = 0; i <= iterations; i++) {
        State s = {.api=&api, .data=data, .length=(size_t)length, .chunk=chunk, .hash=UINT64_C(14695981039346656037), .trace=argc == 6};
        printf("%s{\"trace\":\"", i ? "," : "");
        double before = now();
        XML_Parser root = api.create(NULL);
        if (!root) return 1;
        s.active = root;
        api.user_data(root, &s); api.external(root, external);
        if (!api.parameters(root, XML_PARAM_ENTITY_PARSING_ALWAYS)) { api.destroy(root); return 1; }
        api.elements(root, start, end); api.entity(root, entity);
        api.attlist(root, attlist); api.model(root, model);
        const char document[] = "<!DOCTYPE r SYSTEM 'd'><r/>";
        int result = feed(&s, root, document, sizeof(document) - 1);
        api.destroy(root);
        double elapsed = now() - before;
        if (!result) return 1;
        if (!i) expected = s.hash;
        if (s.hash != expected) { fprintf(stderr, "unstable callbacks\n"); return 1; }
        printf("\",\"warmup\":%s,\"seconds\":%.9f,\"hash\":\"%016" PRIx64 "\",\"declarations\":%zu,\"external_calls\":%zu}",
               i ? "false" : "true", elapsed, s.hash, s.declarations, s.external_calls);
    }
    puts("]}"); free(data); dlclose(library);
    return 0;
}

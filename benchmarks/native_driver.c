/* Repeated XML_Parse calls with native callbacks; no Python callback overhead. */
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

typedef void *Parser;
typedef struct {
    Parser (*create)(const char *);
    void (*destroy)(Parser);
    int (*parse)(Parser, const char *, int, int);
    void (*user_data)(Parser, void *);
    void (*element_handler)(Parser, void (*)(void *, const char *, const char **), void (*)(void *, const char *));
    void (*text_handler)(Parser, void (*)(void *, const char *, int));
    const char *(*version)(void);
    int (*error)(Parser);
} Api;
typedef struct { uint64_t hash; size_t elements; size_t text_bytes; } State;

static void hash_bytes(State *state, const char *value, size_t length) {
    for (size_t i = 0; i < length; i++) {
        state->hash ^= (unsigned char)value[i];
        state->hash *= UINT64_C(1099511628211);
    }
}
static void start(void *context, const char *name, const char **attributes) {
    State *state = context;
    hash_bytes(state, "\xffS", 2);
    hash_bytes(state, name, strlen(name));
    for (size_t i = 0; attributes[i]; i++) {
        hash_bytes(state, "\0", 1);
        hash_bytes(state, attributes[i], strlen(attributes[i]));
    }
    hash_bytes(state, "\0", 1);
    state->elements++;
}
static void end(void *context, const char *name) {
    State *state = context;
    hash_bytes(state, "\xff" "E", 2);
    hash_bytes(state, name, strlen(name));
    hash_bytes(state, "\0", 1);
}
static void text(void *context, const char *value, int length) {
    State *state = context;
    hash_bytes(state, value, (size_t)length);
    state->text_bytes += (size_t)length;
}
static double now(void) {
    struct timespec stamp;
    if (clock_gettime(CLOCK_MONOTONIC, &stamp)) abort();
    return (double)stamp.tv_sec + (double)stamp.tv_nsec / 1e9;
}
static size_t positive(const char *value) {
    char *endptr = NULL;
    errno = 0;
    unsigned long parsed = strtoul(value, &endptr, 10);
    if (errno || *endptr || !parsed || parsed > INT_MAX) {
        fprintf(stderr, "expected positive integer <= INT_MAX: %s\n", value);
        exit(2);
    }
    return (size_t)parsed;
}
#define LOAD(field, symbol) do { \
    void *address = dlsym(library, symbol); \
    if (!address) { fprintf(stderr, "missing %s: %s\n", symbol, dlerror()); return 2; } \
    memcpy(&api.field, &address, sizeof(address)); \
} while (0)
int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr, "usage: %s LIBRARY XML_FILE CHUNK_SIZE ITERATIONS\n", argv[0]);
        return 2;
    }
    size_t chunk_size = positive(argv[3]), iterations = positive(argv[4]);
    FILE *input = fopen(argv[2], "rb");
    if (!input) { perror("fopen"); return 2; }
    if (fseek(input, 0, SEEK_END)) return 2;
    long length = ftell(input);
    if (length < 0 || fseek(input, 0, SEEK_SET)) return 2;
    char *data = malloc((size_t)length + 1);
    if (!data || fread(data, 1, (size_t)length, input) != (size_t)length) return 2;
    fclose(input);
    void *library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "%s\n", dlerror()); return 2; }
    Api api;
    LOAD(create, "XML_ParserCreate");
    LOAD(destroy, "XML_ParserFree");
    LOAD(parse, "XML_Parse");
    LOAD(user_data, "XML_SetUserData");
    LOAD(element_handler, "XML_SetElementHandler");
    LOAD(text_handler, "XML_SetCharacterDataHandler");
    LOAD(version, "XML_ExpatVersion");
    LOAD(error, "XML_GetErrorCode");
    /* Version is a trusted ASCII implementation string. */
    printf("{\"version\":\"%s\",\"samples\":[", api.version());
    uint64_t expected_hash = 0;
    for (size_t iteration = 0; iteration <= iterations; iteration++) {
        State state = {UINT64_C(14695981039346656037), 0, 0};
        double before = now();
        Parser parser = api.create(NULL);
        if (!parser) { fprintf(stderr, "parser allocation failed\n"); return 1; }
        api.user_data(parser, &state);
        api.element_handler(parser, start, end);
        api.text_handler(parser, text);
        size_t offset = 0;
        do {
            size_t count = (size_t)length - offset;
            if (count > chunk_size) count = chunk_size;
            if (api.parse(parser, data + offset, (int)count, offset + count == (size_t)length) != 1) {
                fprintf(stderr, "parse failed with error %d at input offset %zu\n", api.error(parser), offset);
                api.destroy(parser);
                return 1;
            }
            offset += count;
        } while (offset < (size_t)length);
        api.destroy(parser);
        double elapsed = now() - before;
        if (iteration == 0) expected_hash = state.hash;
        if (state.hash != expected_hash) { fprintf(stderr, "unstable output\n"); return 1; }
        printf("%s{\"iteration\":%zu,\"warmup\":%s,\"seconds\":%.9f,\"hash\":\"%016" PRIx64 "\",\"elements\":%zu,\"text_bytes\":%zu}", iteration ? "," : "", iteration, iteration ? "false" : "true", elapsed, state.hash, state.elements, state.text_bytes);
    }
    puts("]}");
    free(data);
    dlclose(library);
    return 0;
}

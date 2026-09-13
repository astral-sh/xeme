/* Persistent, separately linked Expat 2.8.4 semantic oracle.
 * The process boundary prevents XML_* symbol interposition with Xeme. */
#include <expat.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define MAX_INPUT 8192
#define MAX_OUTPUT (32 * 1024 * 1024)

static unsigned char output[MAX_OUTPUT];
static size_t used, last_text;

static void put32(unsigned char *p, uint32_t value) {
  p[0] = value >> 24;
  p[1] = value >> 16;
  p[2] = value >> 8;
  p[3] = value;
}

static uint32_t get32(const unsigned char *p) {
  return (uint32_t)p[0] << 24 | (uint32_t)p[1] << 16 | (uint32_t)p[2] << 8 | p[3];
}

static void append(const void *bytes, size_t len) {
  if (len > MAX_OUTPUT - used)
    abort();
  memcpy(output + used, bytes, len);
  used += len;
}

static void number(uint32_t value) {
  unsigned char bytes[4];
  put32(bytes, value);
  append(bytes, sizeof(bytes));
}

static void string(const char *value) {
  size_t len = strlen(value);
  number((uint32_t)len);
  append(value, len);
}

static void XMLCALL start(void *data, const XML_Char *name, const XML_Char **attrs) {
  (void)data;
  last_text = SIZE_MAX;
  append("S", 1);
  string(name);
  size_t count = 0;
  while (attrs[count * 2])
    count++;
  number((uint32_t)count);
  for (size_t i = 0; i < count * 2; i++)
    string(attrs[i]);
}

static void XMLCALL end(void *data, const XML_Char *name) {
  (void)data;
  last_text = SIZE_MAX;
  append("E", 1);
  string(name);
}

static void XMLCALL text(void *data, const XML_Char *bytes, int length) {
  (void)data;
  if (last_text == SIZE_MAX) {
    append("T", 1);
    last_text = used;
    number(0);
  }
  put32(output + last_text, get32(output + last_text) + (uint32_t)length);
  append(bytes, (size_t)length);
}

int main(void) {
  if (strcmp(XML_ExpatVersion(), "expat_2.8.4") != 0) {
    fprintf(stderr, "oracle requires Expat 2.8.4, loaded %s\n", XML_ExpatVersion());
    return 2;
  }
  if (fwrite("EXPAT284", 1, 8, stdout) != 8 || fflush(stdout))
    return 2;
  for (;;) {
    unsigned char header[7], input[MAX_INPUT];
    size_t received = fread(header, 1, sizeof(header), stdin);
    if (received == 0 && feof(stdin))
      return 0;
    if (received != sizeof(header))
      return 2;
    size_t length = get32(header + 3);
    size_t width = (size_t)header[1] * 256 + header[2];
    if (length > MAX_INPUT || width == 0 || header[0] > 1)
      return 2;
    if (fread(input, 1, length, stdin) != length)
      return 2;
    alarm(5);
    XML_Parser parser = header[0] ? XML_ParserCreateNS(NULL, '|')
                                 : XML_ParserCreate(NULL);
    if (!parser)
      return 2;
    used = 0;
    last_text = SIZE_MAX;
    XML_SetElementHandler(parser, start, end);
    XML_SetCharacterDataHandler(parser, text);
    enum XML_Status status = XML_STATUS_OK;
    for (size_t offset = 0; offset < length && status == XML_STATUS_OK;) {
      size_t count = length - offset < width ? length - offset : width;
      status = XML_Parse(parser, (const char *)input + offset, (int)count, 0);
      offset += count;
    }
    if (status == XML_STATUS_OK)
      status = XML_Parse(parser, NULL, 0, 1);
    XML_ParserFree(parser);
    alarm(0);
    unsigned char reply[5];
    reply[0] = status == XML_STATUS_OK;
    put32(reply + 1, (uint32_t)used);
    if (fwrite(reply, 1, sizeof(reply), stdout) != sizeof(reply)
        || fwrite(output, 1, used, stdout) != used || fflush(stdout))
      return 2;
  }
}

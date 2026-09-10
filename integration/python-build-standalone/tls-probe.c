#include <assert.h>
#include <pthread.h>
#include <stddef.h>

extern void initialize(void);
extern size_t drops(void);

static void *run(void *unused) {
  (void)unused;
  initialize();
  return NULL;
}

int main(void) {
  for (size_t i = 0; i < 32; i++) {
    pthread_t thread;
    assert(pthread_create(&thread, NULL, run, NULL) == 0);
    assert(pthread_join(thread, NULL) == 0);
    assert(drops() == i + 1);
  }
}

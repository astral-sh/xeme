/* The guard itself uses the generic ISA; GCC also checks AVX OS-state support. */
#include <stdio.h>

int main(void) {
    __builtin_cpu_init();
    if (!__builtin_cpu_supports("x86-64-v3")) {
        fputs("This PBS trial requires usable x86-64-v3 CPU and OS support.\n", stderr);
        return 1;
    }
    puts("x86-64-v3 CPU and OS support verified");
    return 0;
}

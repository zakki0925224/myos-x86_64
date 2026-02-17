#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wimplicit-function-declaration"

#include "stdio.h"

void _start_c(int argc, char const* argv[]) {
    exit((uint64_t)main(argc, argv));
}

__attribute__((naked)) void _start(void) {
    __asm__ volatile(
        "andq $-16, %%rsp\n"
        "call _start_c\n"
        "hlt\n"
        :
        :
        : "memory");
}

#pragma GCC diagnostic pop

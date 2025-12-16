#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wimplicit-function-declaration"

#include "stdio.h"

void _start(int argc, char const* argv[]) {
    exit((uint64_t)main(argc, argv));
}

#pragma GCC diagnostic pop

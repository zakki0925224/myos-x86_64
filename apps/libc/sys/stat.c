#include "stat.h"

#include <stdint.h>

#include "stdio.h"

int mkdir(const char* path, __mode_t mode) {
    printf("[DEBUG]mkdir called\n");
    return -1;
}

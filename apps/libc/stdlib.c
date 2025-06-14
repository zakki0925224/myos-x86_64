#include "stdlib.h"

#include <stddef.h>

#include "stdio.h"  // for printf
#include "string.h"
#include "syscalls.h"

int abs(int i) {
    return i < 0 ? -i : i;
}

void *malloc(size_t len) {
    return sys_sbrk(len);
}

int atoi(const char *str) {
    printf("[DEBUG]atoi called\n");
    return -1;
}

double atof(const char *nptr) {
    printf("[DEBUG]atof called\n");
    return -1.0;
}

void free(void *ptr) {
    printf("[DEBUG]free called\n");
}

void *calloc(size_t count, size_t size) {
    // printf("[DEBUG]calloc called\n");
    void *ptr = malloc(count * size);
    if (ptr == NULL)
        return NULL;

    memset(ptr, 0, count * size);
    return ptr;
}

void *realloc(void *ptr, size_t size) {
    // printf("[DEBUG]realloc called\n");
    if (ptr == NULL) {
        return malloc(size);
    }

    size_t old_size = sys_sbrksz(ptr);
    if (old_size == 0)
        return NULL;

    void *new_ptr = malloc(size);
    if (new_ptr == NULL)
        return NULL;

    memcpy(new_ptr, ptr, old_size > size ? size : old_size);
    free(ptr);
    return new_ptr;
}

int system(const char *command) {
    printf("[DEBUG]system called (command: %s)\n", command);
    return -1;
}

int remove(const char *filepath) {
    printf("[DEBUG]remove called\n");
    return -1;
}

int rename(const char *old, const char *_new) {
    printf("[DEBUG]rename called\n");
    return -1;
}

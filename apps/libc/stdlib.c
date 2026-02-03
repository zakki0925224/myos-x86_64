#include "stdlib.h"

#include <limits.h>
#include <stddef.h>

#include "ctype.h"
#include "stdio.h"  // for printf
#include "string.h"
#include "syscalls.h"

int abs(int i) {
    return i < 0 ? -i : i;
}

void* malloc(size_t len) {
    return sys_sbrk(len);
}

int atoi(const char* str) {
    printf("[DEBUG]atoi called\n");
    return -1;
}

double atof(const char* nptr) {
    printf("[DEBUG]atof called\n");
    return -1.0;
}

void free(void* ptr) {
    printf("[DEBUG]free called\n");
}

void* calloc(size_t count, size_t size) {
    // printf("[DEBUG]calloc called\n");
    void* ptr = malloc(count * size);
    if (ptr == NULL)
        return NULL;

    memset(ptr, 0, count * size);
    return ptr;
}

void* realloc(void* ptr, size_t size) {
    // printf("[DEBUG]realloc called\n");
    if (ptr == NULL) {
        return malloc(size);
    }

    size_t old_size = sys_sbrksz(ptr);
    if (old_size == 0)
        return NULL;

    void* new_ptr = malloc(size);
    if (new_ptr == NULL)
        return NULL;

    memcpy(new_ptr, ptr, old_size > size ? size : old_size);
    free(ptr);
    return new_ptr;
}

int system(const char* command) {
    printf("[DEBUG]system called (command: %s)\n", command);
    return -1;
}

int remove(const char* filepath) {
    printf("[DEBUG]remove called\n");
    return -1;
}

int rename(const char* old, const char* _new) {
    printf("[DEBUG]rename called\n");
    return -1;
}

char* getenv(const char* name) {
    printf("[DEBUG]getenv called\n");
    return NULL;
}

void abort(void) {
    printf("[DEBUG]abort called\n");
    while (1) {
        __asm__("hlt");
    }
}

long strtol(const char* nptr, char** endptr, int base) {
    const char* s = nptr;
    unsigned long acc;
    int c;
    unsigned long cutoff;
    int neg = 0, any, cutlim;

    // Skip white space
    do {
        c = *s++;
    } while (isspace(c));

    if (c == '-') {
        neg = 1;
        c = *s++;
    } else if (c == '+') {
        c = *s++;
    }

    if ((base == 0 || base == 16) &&
        c == '0' && (*s == 'x' || *s == 'X')) {
        c = s[1];
        s += 2;
        base = 16;
    }
    if (base == 0)
        base = c == '0' ? 8 : 10;

    cutoff = neg ? -(unsigned long)LONG_MIN : LONG_MAX;
    cutlim = cutoff % (unsigned long)base;
    cutoff /= (unsigned long)base;
    for (acc = 0, any = 0;; c = *s++) {
        if (isdigit(c))
            c -= '0';
        else if (isalpha(c))
            c -= isupper(c) ? 'A' - 10 : 'a' - 10;
        else
            break;
        if (c >= base)
            break;
        if (any < 0 || acc > cutoff || (acc == cutoff && c > cutlim))
            any = -1;
        else {
            any = 1;
            acc *= base;
            acc += c;
        }
    }
    if (any < 0) {
        acc = neg ? LONG_MIN : LONG_MAX;
        // errno = ERANGE;
    } else if (neg)
        acc = -acc;
    if (endptr != 0)
        *endptr = (char*)(any ? s - 1 : nptr);
    return (acc);
}

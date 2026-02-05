#include "stdlib.h"

#include <limits.h>
#include <stddef.h>

#include "ctype.h"
#include "stdio.h"  // for printf
#include "string.h"
#include "syscalls.h"

// malloc/free
#define PAGE_SIZE 4096
#define ALIGN 8

typedef struct FreeBlock {
    size_t size;
    struct FreeBlock* next;
} FreeBlock;

// simple first-fit allocator
static FreeBlock* free_list = NULL;

static FreeBlock* request_mem(size_t need) {
    size_t total = (need + sizeof(FreeBlock) + (ALIGN - 1)) & ~(ALIGN - 1);

    if (total < PAGE_SIZE)
        total = PAGE_SIZE;

    void* ptr = sys_sbrk(total);

    if (ptr == (void*)-1)
        return NULL;

    FreeBlock* block = (FreeBlock*)ptr;
    block->size = total;
    block->next = NULL;
    return block;
}

static void split_block(FreeBlock* block, size_t need) {
    size_t remain = block->size - need;

    // if too small
    if (remain <= sizeof(FreeBlock))
        return;

    FreeBlock* new_block = (FreeBlock*)((char*)block + need);
    new_block->size = remain;
    new_block->next = block->next;

    block->size = need;
    block->next = new_block;
}

void* malloc(size_t len) {
    if (len == 0)
        return NULL;

    size_t need = (len + sizeof(FreeBlock) + (ALIGN - 1)) & ~(ALIGN - 1);

    FreeBlock** prev = &free_list;
    FreeBlock* curr = free_list;

    // find block
    while (curr) {
        if (curr->size >= need) {
            split_block(curr, need);

            *prev = curr->next;
            return (void*)((char*)curr + sizeof(FreeBlock));
        }

        prev = &curr->next;
        curr = curr->next;
    }

    FreeBlock* new_block = request_mem(need);
    if (new_block == NULL)
        return NULL;

    split_block(new_block, need);

    if (new_block->next != NULL) {
        FreeBlock* remain = new_block->next;
        remain->next = free_list;
        free_list = remain;

        new_block->next = NULL;
    }

    return (void*)((char*)new_block + sizeof(FreeBlock));
}

void free(void* ptr) {
    if (ptr == NULL)
        return;

    FreeBlock* block = (FreeBlock*)((char*)ptr - sizeof(FreeBlock));
    block->next = free_list;
    free_list = block;
}

void* calloc(size_t count, size_t size) {
    void* ptr = malloc(count * size);
    if (ptr == NULL)
        return NULL;

    memset(ptr, 0, count * size);
    return ptr;
}

void* realloc(void* ptr, size_t size) {
    if (ptr == NULL) {
        return malloc(size);
    }

    if (size == 0) {
        free(ptr);
        return NULL;
    }

    FreeBlock* block = (FreeBlock*)((char*)ptr - sizeof(FreeBlock));
    size_t old_size = block->size - sizeof(FreeBlock);

    if (size <= old_size) {
        return ptr;
    }

    void* new_ptr = malloc(size);
    if (!new_ptr) return NULL;

    memcpy(new_ptr, ptr, old_size < size ? old_size : size);
    free(ptr);
    return new_ptr;
}

// --------------------------------------------------------------

int abs(int i) {
    return i < 0 ? -i : i;
}

int atoi(const char* str) {
    printf("[DEBUG]atoi called\n");
    return -1;
}

double atof(const char* nptr) {
    printf("[DEBUG]atof called\n");
    return -1.0;
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

#include "locale.h"

#include <stddef.h>

#include "stdio.h"

char* setlocale(int category, const char* locale) {
    printf("[DEBUG]setlocale called\n");
    return "C";
}

struct lconv* localeconv(void) {
    static struct lconv l;
    return &l;
}

#include "locale.h"

#include <stddef.h>

char* setlocale(int category, const char* locale) {
    return "C";
}

struct lconv* localeconv(void) {
    static struct lconv l;
    return &l;
}

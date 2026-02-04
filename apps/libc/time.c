#include "time.h"

#include <stdio.h>

#include "syscalls.h"

time_t time(time_t* t) {
    printf("[DEBUG]time called\n");
    return 0;
}

clock_t clock(void) {
    printf("[DEBUG]clock called\n");
    return 0;
}

double difftime(time_t time1, time_t time0) {
    return (double)(time1 - time0);
}

time_t mktime(struct tm* timeptr) {
    printf("[DEBUG]mktime called\n");
    return 0;
}

size_t strftime(char* restrict s, size_t maxsize, const char* restrict format, const struct tm* restrict timeptr) {
    printf("[DEBUG]strftime called\n");
    if (maxsize > 0 && s != NULL) {
        s[0] = '\0';
    }
    return 0;
}

struct tm* gmtime(const time_t* timer) {
    printf("[DEBUG]gmtime called\n");
    static struct tm t = {0};
    return &t;
}

struct tm* localtime(const time_t* timer) {
    printf("[DEBUG]localtime called\n");
    static struct tm t = {0};
    return &t;
}

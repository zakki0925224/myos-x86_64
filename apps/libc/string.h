#ifndef _STRING_H
#define _STRING_H

#include <stddef.h>

extern int strcmp(const char* s1, const char* s2);
extern size_t strlen(const char* str);
extern int split(char* str, const char regex, char** buf, size_t buflen);
extern char* concatenate(const char* strs[], int len, const char* delimiter);
extern void replace(char* src, const char target, const char replace);
extern int is_ascii(const char c);
extern void* memcpy(void* dest, const void* src, size_t len);
extern void* memset(void* dest, int val, size_t len);
extern void* memmove(void* dest, const void* src, size_t len);
extern int strcasecmp(const char* s1, const char* s2);
extern int strncasecmp(const char* s1, const char* s2, size_t n);
extern char* strchr(const char* s1, int i);
extern char* strrchr(const char* s, int i);
extern int strncmp(const char* s1, const char* s2, size_t n);
extern char* strncpy(char* dst, const char* src, size_t n);
extern char* strdup(const char* s);
extern char* strstr(const char* s1, const char* s2);

#endif

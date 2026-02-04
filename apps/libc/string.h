#ifndef _STRING_H
#define _STRING_H

#include <stddef.h>

int strcmp(const char* s1, const char* s2);
size_t strlen(const char* str);
int split(char* str, const char regex, char** buf, size_t buflen);
char* concatenate(const char* strs[], int len, const char* delimiter);
void replace(char* src, const char target, const char replace);
int is_ascii(const char c);
int memcmp(const void* s1, const void* s2, size_t n);
void* memcpy(void* dest, const void* src, size_t len);
void* memset(void* dest, int val, size_t len);
void* memmove(void* dest, const void* src, size_t len);
int strcasecmp(const char* s1, const char* s2);
int strncasecmp(const char* s1, const char* s2, size_t n);
char* strchr(const char* s1, int i);
char* strrchr(const char* s, int i);
int strncmp(const char* s1, const char* s2, size_t n);
char* strcpy(char* dest, const char* src);
char* strncpy(char* dst, const char* src, size_t n);
char* strdup(const char* s);
char* strstr(const char* s1, const char* s2);
size_t strspn(const char* s, const char* accept);
char* strpbrk(const char* s, const char* accept);
char* strerror(int errnum);

#endif

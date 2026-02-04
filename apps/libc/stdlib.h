#ifndef _STDLIB_H
#define _STDLIB_H

#include <stddef.h>

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

int abs(int i);
void* malloc(size_t len);
int atoi(const char* str);
double atof(const char* nptr);
void free(void* ptr);
void* calloc(size_t count, size_t size);
void* realloc(void* ptr, size_t size);
int system(const char* command);
int remove(const char* filepath);
int rename(const char* old, const char* _new);
char* getenv(const char* name);
void abort(void);
long strtol(const char* nptr, char** endptr, int base);

#endif

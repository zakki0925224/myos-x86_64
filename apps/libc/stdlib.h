#ifndef _STDLIB_H
#define _STDLIB_H

#include <stddef.h>

#define EXIT_SUCCESS 0
#define EXIT_FAILURE 1

extern int abs(int i);
extern void* malloc(size_t len);
extern int atoi(const char* str);
extern double atof(const char* nptr);
extern void free(void* ptr);
extern void* calloc(size_t count, size_t size);
extern void* realloc(void* ptr, size_t size);
extern int system(const char* command);
extern int remove(const char* filepath);
extern int rename(const char* old, const char* _new);
extern char* getenv(const char* name);
extern void abort(void);
extern long strtol(const char* nptr, char** endptr, int base);

#endif

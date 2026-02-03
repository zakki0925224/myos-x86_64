#ifndef _STDIO_H
#define _STDIO_H

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

#include "stat.h"

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

#define _IONBF 0
#define _IOLBF 1
#define _IOFBF 2

#define EOF (-1)
#define BUFSIZ 1024

typedef struct
{
    int fd;
    f_stat* stat;
    char* buf;
    long int pos;
} FILE;

extern FILE* stdin;
extern FILE* stdout;
extern FILE* stderr;

// printf.c
extern int printf(const char* fmt, ...);

extern void exit(int status);
extern int fprintf(FILE* stream, const char* fmt, ...);
extern int snprintf(char* buf, size_t size, const char* format, ...);
extern FILE* fopen(const char* filepath, const char* mode);
extern int fclose(FILE* stream);
extern long int ftell(FILE* stream);
extern int fflush(FILE* __stream);
extern int puts(const char* c);
extern int putchar(int c);
extern char getchar(void);
extern int vfprintf(FILE* stream, const char* fmt, va_list ap);
extern int sscanf(const char* buf, const char* fmt, ...);
extern size_t fread(void* buf, size_t size, size_t count, FILE* stream);
extern int fseek(FILE* stream, long int offset, int whence);
extern size_t fwrite(const void* buf, size_t size, size_t count, FILE* stream);
extern int vsnprintf(char* buf, size_t bufsize, const char* format, va_list arg);
extern int setvbuf(FILE* stream, char* buf, int mode, size_t size);
extern void clearerr(FILE* stream);
extern int ferror(FILE* stream);
extern int feof(FILE* stream);
extern FILE* tmpfile(void);
extern int ungetc(int c, FILE* stream);
extern int getc(FILE* stream);
extern char* fgets(char* s, int size, FILE* stream);
extern FILE* freopen(const char* filename, const char* mode, FILE* stream);
extern int fputs(const char* s, FILE* stream);

#endif

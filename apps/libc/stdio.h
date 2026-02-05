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

#define _FILE_EOF_FLAG 0x01
#define _FILE_ERR_FLAG 0x02

typedef struct
{
    int fd;
    f_stat* stat;
    char* buf;
    long int pos;
    int flags;
} FILE;

extern FILE* stdin;
extern FILE* stdout;
extern FILE* stderr;

// printf.c
int printf(const char* fmt, ...);

void exit(int status);
int fprintf(FILE* stream, const char* fmt, ...);
int snprintf(char* buf, size_t size, const char* format, ...);
FILE* fopen(const char* filepath, const char* mode);
int fclose(FILE* stream);
long int ftell(FILE* stream);
int fflush(FILE* __stream);
int puts(const char* c);
int putchar(int c);
char getchar(void);
int vfprintf(FILE* stream, const char* fmt, va_list ap);
int sscanf(const char* buf, const char* fmt, ...);
size_t fread(void* buf, size_t size, size_t count, FILE* stream);
int fseek(FILE* stream, long int offset, int whence);
size_t fwrite(const void* buf, size_t size, size_t count, FILE* stream);
int vsnprintf(char* buf, size_t bufsize, const char* format, va_list arg);
int setvbuf(FILE* stream, char* buf, int mode, size_t size);
void clearerr(FILE* stream);
int ferror(FILE* stream);
int feof(FILE* stream);
FILE* tmpfile(void);
int ungetc(int c, FILE* stream);
int getc(FILE* stream);
char* fgets(char* s, int size, FILE* stream);
FILE* freopen(const char* filename, const char* mode, FILE* stream);
int fputs(const char* s, FILE* stream);

#endif

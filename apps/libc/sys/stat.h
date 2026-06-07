#ifndef _SYS_STAT_H
#define _SYS_STAT_H

#include <stddef.h>
#include <sys/types.h>

struct stat {
    size_t st_size;
};

typedef struct
{
    size_t size;
} f_stat;

int mkdir(const char* path, mode_t mode);
int stat(const char* path, struct stat* buf);

#endif

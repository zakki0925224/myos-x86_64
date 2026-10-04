#ifndef _POLL_H
#define _POLL_H

#include <stdint.h>

#define POLLIN 0x1

typedef struct {
    int fd;
    short events;
    short revents;
} pollfd;

#endif

#include "unistd.h"

#include "syscalls.h"

int chdir(const char* path) {
    return sys_chdir(path);
}

unsigned int sleep(unsigned int seconds) {
    sys_sleep((uint64_t)seconds * 1000);
    return 0;
}

int usleep(unsigned int usec) {
    sys_sleep(((uint64_t)usec + 999) / 1000);
    return 0;
}

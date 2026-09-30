#include "unistd.h"

#include "syscalls.h"

int chdir(const char* path) {
    return sys_chdir(path);
}

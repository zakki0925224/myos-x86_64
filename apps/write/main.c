#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>

int main(int argc, char *argv[]) {
    if (argc < 3) {
        return 0;
    }

    int64_t fd = sys_open(argv[1], OPEN_FLAG_CREATE);

    if (fd == -1) {
        printf("write: failed to open the file\n");
        return 1;
    }

    if (sys_write(fd, argv[2], strlen(argv[2])) == -1) {
        printf("write: failed to write to the file\n");
        return 1;
    }

    if (sys_close(fd) == -1) {
        printf("write: failed to close the file\n");
        return 1;
    }

    return 0;
}

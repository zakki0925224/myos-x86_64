#include <stat.h>
#include <stdio.h>
#include <stdlib.h>
#include <syscalls.h>

int main(int argc, char *argv[]) {
    int64_t fd = sys_open("/dev/pci-bus", OPEN_FLAG_NONE);

    if (fd == -1) {
        printf("lspci: failed to open the file\n");
        return 1;
    }

    f_stat *file_stat = (f_stat *)malloc(sizeof(f_stat));
    if (sys_stat(fd, file_stat) == -1) {
        printf("cat: failed to get the file status\n");
        return 1;
    }

    char *f_buf = (char *)malloc(file_stat->size);
    if (sys_read(fd, f_buf, file_stat->size) == -1) {
        printf("cat: failed to read the file\n");
        return 1;
    }

    if (sys_close(fd) == -1) {
        printf("cat: failed to close the file\n");
        return 1;
    }

    f_buf[file_stat->size] = '\0';  // Null-terminate the string
    printf("%s\n", f_buf);

    return 0;
}

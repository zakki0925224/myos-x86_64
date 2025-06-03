#include <stat.h>
#include <stdio.h>
#include <stdlib.h>
#include <syscalls.h>

int main(int argc, char *argv[]) {
    if (argc < 2) {
        return 0;
    }

    int64_t fd = sys_open(argv[1], OPEN_FLAG_NONE);

    if (fd == -1) {
        printf("hexdump: failed to open the file\n");
        return -1;
    }

    f_stat *file_stat = (f_stat *)malloc(sizeof(f_stat));
    if (sys_stat(fd, file_stat) == -1) {
        printf("hexdump: failed to get the file status\n");
        return -1;
    }

    char *f_buf = (char *)malloc(file_stat->size);
    if (sys_read(fd, f_buf, file_stat->size) == -1) {
        printf("hexdump: failed to read the file\n");
        return -1;
    }

    if (sys_close(fd) == -1) {
        printf("hexdump: failed to close the file\n");
        return -1;
    }

    for (int i = 0; i < (file_stat->size + 15) / 16; i++) {
        int j = i * 16;
        int j_end = j + 16;

        if (j_end > file_stat->size) {
            j_end = file_stat->size;
        }

        printf("%08x ", i * 16);

        for (; j < j_end; j++) {
            if (j % 2 == 0) {
                printf(" ");
            }

            printf("%02x ", f_buf[j]);
        }

        if (j_end < 16) {
            for (int k = 0; k < 16 - j_end; k++) {
                printf("   ");
            }
            printf(" ");
        }

        printf(" |");
        for (int j = i * 16; j < j_end; j++) {
            // printable characters
            if (f_buf[j] >= 0x20 && f_buf[j] <= 0x7e) {
                printf("%c", f_buf[j]);
            } else {
                printf(".");
            }
        }
        printf("|\n");
    }

    printf("\n");
    return 0;
}

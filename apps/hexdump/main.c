#include <stdio.h>
#include <stdlib.h>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        return 0;
    }

    FILE* file = fopen(argv[1], "r");
    if (file == NULL) {
        printf("hexdump: failed to open the file\n");
        return -1;
    }

    size_t file_size = file->stat->size;

    char* f_buf = (char*)malloc(file_size);
    if (f_buf == NULL) {
        printf("hexdump: failed to allocate memory\n");
        fclose(file);
        return -1;
    }

    fread(f_buf, 1, file_size, file);
    fclose(file);

    for (int i = 0; i < ((int)file_size + 15) / 16; i++) {
        int j = i * 16;
        int j_end = j + 16;

        if (j_end > (int)file_size) {
            j_end = (int)file_size;
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
    free(f_buf);
    return 0;
}

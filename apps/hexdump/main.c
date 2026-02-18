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

    unsigned char* f_buf = (unsigned char*)malloc(file_size);
    if (f_buf == NULL) {
        printf("hexdump: failed to allocate memory\n");
        fclose(file);
        return -1;
    }

    fread(f_buf, 1, file_size, file);
    fclose(file);

    for (int i = 0; i < ((int)file_size + 15) / 16; i++) {
        int line_start = i * 16;

        printf("%08x ", line_start);

        for (int k = 0; k < 16; k++) {
            int pos = line_start + k;

            if (k % 2 == 0) {
                printf(" ");
            }

            if (pos < (int)file_size) {
                printf("%02x ", f_buf[pos]);
            } else {
                printf("   ");
            }
        }

        printf(" |");
        for (int k = 0; k < 16; k++) {
            int pos = line_start + k;

            if (pos < (int)file_size) {
                // printable characters
                if (f_buf[pos] >= 0x20 && f_buf[pos] <= 0x7e) {
                    printf("%c", f_buf[pos]);
                } else {
                    printf(".");
                }
            }
        }
        printf("|\n");
    }

    printf("\n");
    free(f_buf);
    return 0;
}

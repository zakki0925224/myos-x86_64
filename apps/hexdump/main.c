#include <stdio.h>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        return 0;
    }

    FILE* file = fopen(argv[1], "r");
    if (file == NULL) {
        printf("hexdump: failed to open the file\n");
        return -1;
    }

    unsigned char line[16];
    int line_start = 0;
    size_t n;

    while ((n = fread(line, 1, sizeof(line), file)) > 0) {
        printf("%08x ", line_start);

        for (int k = 0; k < 16; k++) {
            if (k % 2 == 0) {
                printf(" ");
            }

            if ((size_t)k < n) {
                printf("%02x ", line[k]);
            } else {
                printf("   ");
            }
        }

        printf(" |");
        for (size_t k = 0; k < n; k++) {
            // printable characters
            if (line[k] >= 0x20 && line[k] <= 0x7e) {
                printf("%c", line[k]);
            } else {
                printf(".");
            }
        }
        printf("|\n");

        line_start += (int)n;
    }

    printf("\n");
    fclose(file);
    return 0;
}

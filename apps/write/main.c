#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char *argv[]) {
    if (argc < 3) {
        return 0;
    }

    FILE *file = fopen(argv[1], "w");
    if (file == NULL) {
        printf("write: failed to open the file\n");
        return -1;
    }

    if (fwrite(argv[2], 1, strlen(argv[2]), file) != strlen(argv[2])) {
        printf("write: failed to write to the file\n");
        fclose(file);
        return -1;
    }

    if (fflush(file) == -1) {
        printf("write: failed to flush the file\n");
        fclose(file);
        return -1;
    }

    if (fclose(file) == -1) {
        printf("write: failed to close the file\n");
        return -1;
    }

    return 0;
}

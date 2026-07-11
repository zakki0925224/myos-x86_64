#include <stdio.h>

int main(int argc, char* argv[]) {
    FILE* file = fopen("/dev/pci-bus", "r");

    if (file == NULL) {
        printf("lspci: failed to open the file\n");
        return -1;
    }

    char chunk[512];
    size_t n;
    while ((n = fread(chunk, 1, sizeof(chunk), file)) > 0) {
        fwrite(chunk, 1, n, stdout);
    }

    printf("\n");
    fclose(file);

    return 0;
}

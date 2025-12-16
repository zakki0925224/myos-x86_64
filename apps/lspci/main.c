#include <stdio.h>
#include <stdlib.h>

int main(int argc, char* argv[]) {
    FILE* file = fopen("/dev/pci-bus", "r");

    if (file == NULL) {
        printf("lspci: failed to open the file\n");
        return -1;
    }

    size_t file_size = file->stat->size;

    char* f_buf = (char*)malloc(file_size + 1);
    if (f_buf == NULL) {
        printf("lspci: failed to allocate memory\n");
        fclose(file);
        return -1;
    }

    fread(f_buf, 1, file_size, file);
    fclose(file);

    f_buf[file_size] = '\0';
    printf("%s\n", f_buf);

    free(f_buf);

    return 0;
}

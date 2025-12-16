#include <stdio.h>
#include <stdlib.h>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        return 0;
    }

    FILE* file = fopen(argv[1], "r");
    if (file == NULL) {
        printf("cat: failed to open the file\n");
        return -1;
    }

    size_t file_size = file->stat->size;
    char* buf = (char*)malloc(file_size);
    if (buf == NULL) {
        printf("cat: failed to allocate memory\n");
        fclose(file);
        return -1;
    }
    fread(buf, 1, file_size, file);

    printf("%s\n", buf);
    fclose(file);
    free(buf);

    return 0;
}

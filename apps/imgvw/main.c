#include <stdio.h>
#include <stdlib.h>
#include <syscalls.h>
#include <window.h>

#include "libbmp.h"

int main(int argc, char *argv[]) {
    if (argc < 2) {
        printf("Usage: imgvw <filename>\n");
        return -1;
    }

    ComponentDescriptor *cdesc = create_component_window("Imgvw", 50, 50, 500, 300);
    if (cdesc == NULL) {
        printf("Failed to create window\n");
        return -1;
    }

    bmp_image_t *image = bmp_load(argv[1]);
    if (image == NULL) {
        printf("Failed to load image: %s\n", argv[1]);
        if (remove_component(cdesc) == -1) {
            printf("Failed to remove window\n");
        }
        free(cdesc);
        return -1;
    }

    printf("Enter any key to exit...\n");
    char input_key = '\0';
    for (;;) {
        sys_read(FDN_STDIN, &input_key, 1);
        if (input_key != '\0') {
            break;
        }
    }

    if (remove_component(cdesc) == -1) {
        printf("Failed to remove window\n");
        free(cdesc);
        return -1;
    }

    free(cdesc);
    return 0;
}

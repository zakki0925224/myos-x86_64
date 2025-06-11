#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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

    int bmp_row_bytes = ((image->width * image->bytes_per_pixel + 3) / 4) * 4;
    uint8_t *framebuf = malloc(image->width * image->height * image->bytes_per_pixel);
    if (!framebuf) {
        printf("Failed to allocate frame buffer\n");
        bmp_free(image);
        free(cdesc);
        return -1;
    }

    for (size_t y = 0; y < image->height; ++y) {
        size_t src_y = image->height - 1 - y;
        memcpy(
            framebuf + y * image->width * image->bytes_per_pixel,
            image->data + src_y * bmp_row_bytes,
            image->width * image->bytes_per_pixel);
    }

    ComponentDescriptor *img_desc = create_component_image(
        cdesc,
        image->width,
        image->height,
        PIXEL_FORMAT_BGR,
        framebuf);
    if (!img_desc) {
        printf("Failed to create image component\n");
        free(framebuf);
        bmp_free(image);
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

    free(framebuf);
    bmp_free(image);
    if (remove_component(cdesc) == -1) {
        printf("Failed to remove window\n");
        free(cdesc);
        return -1;
    }

    free(cdesc);
    return 0;
}

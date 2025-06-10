#include "libbmp.h"

#include <stdio.h>
#include <stdlib.h>

bmp_image_t *bmp_load(const char *filename) {
    FILE *file = fopen(filename, "");

    if (file == NULL) {
        printf("Failed to open file: %s\n", filename);
        return NULL;
    }

    bmp_file_t bmp_file;
    if (fread(&bmp_file, sizeof(bmp_file), 1, file) == 0) {
        printf("Failed to read BMP header from file: %s\n", filename);
        fclose(file);
        return NULL;
    }

    if (bmp_file.header.magic != MAGIC) {
        printf("Invalid BMP file: %s\n", filename);
        fclose(file);
        return NULL;
    }

    bmp_image_t *image = (bmp_image_t *)malloc(sizeof(bmp_image_t));
    if (image == NULL) {
        printf("Failed to allocate memory for BMP image\n");
        fclose(file);
        return NULL;
    }

    image->width = bmp_file.info_header.width;
    image->height = bmp_file.info_header.height;
    image->bytes_per_pixel = bmp_file.info_header.bits_per_pixel / 8;
    size_t image_size = image->width * image->height * image->bytes_per_pixel;
    image->data = (uint8_t *)malloc(image_size);
    if (image->data == NULL) {
        printf("Failed to allocate memory for BMP image data\n");
        free(image);
        fclose(file);
        return NULL;
    }

    fseek(file, bmp_file.header.data_offset, SEEK_SET);
    size_t bytes_read = fread(image->data, 1, image_size, file);
    fclose(file);

    if (bytes_read != image_size) {
        printf("Failed to read complete BMP image data from file: %s\n", filename);
        free(image->data);
        free(image);
        return NULL;
    }

    return image;
}

void bmp_free(bmp_image_t *image) {
    if (image != NULL) {
        if (image->data != NULL) {
            free(image->data);
        }
        free(image);
    }
}

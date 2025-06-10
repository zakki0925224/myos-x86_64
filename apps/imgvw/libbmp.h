#ifndef LIBBMP_H
#define LIBBMP_H

#include <stddef.h>
#include <stdint.h>

#define MAGIC 0x4d42  // 'BM'

typedef struct {
    uint16_t magic;
    uint32_t file_size;
    uint32_t _reserved;
    uint32_t data_offset;
} bmp_header_t;

typedef struct {
    uint32_t header_size;
    int32_t width;
    int32_t height;
    uint16_t planes;
    uint16_t bits_per_pixel;
    uint32_t compression;
    uint32_t image_size;
    int32_t x_pixels_per_meter;
    int32_t y_pixels_per_meter;
    uint32_t colors_used;
    uint32_t important_colors;
} bmp_info_header_t;

typedef struct {
    bmp_header_t header;
    bmp_info_header_t info_header;
} bmp_file_t;

typedef struct {
    uint8_t *data;
    size_t width;
    size_t height;
    size_t bytes_per_pixel;
} bmp_image_t;

bmp_image_t *bmp_load(const char *filename);
void bmp_free(bmp_image_t *image);

#endif

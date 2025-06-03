#ifndef _IOMSG_H
#define _IOMSG_H

#include <stddef.h>
#include <stdint.h>

#define IOMSG_CMD_CREATE_WINDOW 0x80000000
#define IOMSG_CMD_DESTROY_WINDOW 0x80000001
#define IOMSG_CMD_ADD_IMAGE_TO_WINDOW 0x80000002

typedef struct {
    uint32_t cmd_id;
    uint32_t payload_size;
} iomsg_header;

typedef struct {
    iomsg_header header;
    size_t x_pos;
    size_t y_pos;
    size_t width;
    size_t height;
    char title[];
} __attribute__((aligned(8))) iomsg_create_window;

typedef struct {
    iomsg_header header;
    int layer_id;
} __attribute__((aligned(8))) iomsg_reply_create_window;

typedef struct {
    iomsg_header header;
    int layer_id;
} __attribute__((aligned(8))) iomsg_destroy_window;

typedef struct {
    iomsg_header header;
} __attribute__((aligned(8))) iomsg_reply_destroy_window;

typedef struct {
    iomsg_header header;
    int layer_id;  // window descriptor
    char _reserved0[4];
    size_t image_width;
    size_t image_height;
    uint8_t pixel_format;
    char _reserved1[7];
    const void *framebuf;
} __attribute__((aligned(8))) iomsg_add_image_to_window;

typedef struct {
    iomsg_header header;
    int layer_id;
} __attribute__((aligned(8))) iomsg_reply_add_image_to_window;

#endif

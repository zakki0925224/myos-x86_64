#ifndef _IOMSG_H
#define _IOMSG_H

#include <stddef.h>
#include <stdint.h>

#define IOMSG_CMD_REMOVE_COMPONENT 0x80000000
#define IOMSG_CMD_CREATE_COMPONENT_WINDOW 0x80000001
#define IOMSG_CMD_CREATE_COMPONENT_IMAGE 0x80000002
#define IOMSG_CMD_CREATE_LAYER 0x80000003
#define IOMSG_CMD_MOVE_LAYER 0x80000004
#define IOMSG_CMD_GET_SCREEN_SIZE 0x80000005

#define LAYER_FLAG_ALWAYS_ON_TOP 0x1

typedef struct {
    uint32_t cmd_id;
    uint32_t payload_size;
} iomsg_header;

typedef struct {
    iomsg_header header;
} __attribute__((aligned(8))) _iomsg_with_header_only;

typedef struct {
    iomsg_header header;
    int layer_id;
} __attribute__((aligned(8))) _iomsg_with_layer_id;

typedef _iomsg_with_layer_id iomsg_remove_component;
typedef _iomsg_with_header_only iomsg_reply_remove_component;

typedef struct {
    iomsg_header header;
    size_t x_pos;
    size_t y_pos;
    size_t width;
    size_t height;
    char title[];
} __attribute__((aligned(8))) iomsg_create_component_window;
typedef _iomsg_with_layer_id iomsg_reply_create_component;

typedef struct {
    iomsg_header header;
    int layer_id;
    char _reserved0[4];
    size_t image_width;
    size_t image_height;
    uint8_t pixel_format;
    char _reserved1[7];
    const void* framebuf;
} __attribute__((aligned(8))) iomsg_create_component_image;

typedef struct {
    iomsg_header header;
    size_t x;
    size_t y;
    size_t width;
    size_t height;
    const void* framebuf;
    uint32_t flags;
    char _reserved[4];
} __attribute__((aligned(8))) iomsg_create_layer;

typedef struct {
    iomsg_header header;
    int layer_id;
    char _reserved[4];
    size_t x;
    size_t y;
} __attribute__((aligned(8))) iomsg_move_layer;

typedef _iomsg_with_header_only iomsg_get_screen_size;

typedef struct {
    iomsg_header header;
    size_t width;
    size_t height;
} __attribute__((aligned(8))) iomsg_reply_get_screen_size;

#endif

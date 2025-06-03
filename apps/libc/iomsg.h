#ifndef _IOMSG_H
#define _IOMSG_H

#define IOMSG_CMD_CREATE_WINDOW 0x80000000
#define IOMSG_CMD_DESTROY_WINDOW 0x80000001

typedef struct {
    uint32_t cmd_id;
    uint32_t payload_size;
} iomsg_header;

typedef struct {
    iomsg_header header;
    uint64_t x_pos;
    uint64_t y_pos;
    uint64_t width;
    uint64_t height;
    char title[];
} __attribute__((aligned(8))) iomsg_create_window;

typedef struct {
    iomsg_header header;
    int64_t layer_id;
} __attribute__((aligned(8))) iomsg_reply_create_window;

#endif

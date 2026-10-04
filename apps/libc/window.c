#include "window.h"

#include "stdio.h"
#include "stdlib.h"
#include "string.h"
#include "syscalls.h"

int remove_component(component_descriptor* cdesc) {
    if (cdesc == NULL) {
        return -1;
    }

    void* msgbuf = malloc(sizeof(iomsg_remove_component));
    if (msgbuf == NULL) {
        return -1;
    }

    iomsg_remove_component* msg = (iomsg_remove_component*)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_REMOVE_COMPONENT;
    msg->header.payload_size = sizeof(int);
    msg->layer_id = cdesc->layer_id;

    void* replymsgbuf = malloc(sizeof(iomsg_reply_remove_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return -1;
    }

    iomsg_reply_remove_component* replymsg = (iomsg_reply_remove_component*)replymsgbuf;
    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_remove_component)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_REMOVE_COMPONENT) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    free(msgbuf);
    free(replymsgbuf);
    free(cdesc);
    return 0;
}

component_descriptor* create_component_window(const char* title, size_t x_pos, size_t y_pos, size_t width, size_t height) {
    size_t title_len = strlen(title) + 1;
    void* msgbuf = malloc(sizeof(iomsg_create_component_window) + title_len);
    if (msgbuf == NULL) {
        return NULL;
    }

    iomsg_create_component_window* msg = (iomsg_create_component_window*)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_COMPONENT_WINDOW;
    msg->header.payload_size = 8 * 4 + title_len;
    msg->x_pos = x_pos;
    msg->y_pos = y_pos;
    msg->width = width;
    msg->height = height;
    memcpy(msg->title, title, title_len);

    void* replymsgbuf = malloc(sizeof(iomsg_reply_create_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return NULL;
    }

    iomsg_reply_create_component* replymsg = (iomsg_reply_create_component*)replymsgbuf;

    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_create_component)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_CREATE_COMPONENT_WINDOW) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    component_descriptor* cdesc = (component_descriptor*)malloc(sizeof(component_descriptor));
    if (cdesc == NULL) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    cdesc->layer_id = replymsg->layer_id;

    free(msgbuf);
    free(replymsgbuf);
    return cdesc;
}

component_descriptor* create_component_image(component_descriptor* cdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void* framebuf) {
    if (cdesc == NULL || framebuf == NULL) {
        return NULL;
    }

    void* msgbuf = malloc(sizeof(iomsg_create_component_image));
    if (msgbuf == NULL) {
        return NULL;
    }

    iomsg_create_component_image* msg = (iomsg_create_component_image*)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_COMPONENT_IMAGE;
    msg->header.payload_size = sizeof(iomsg_create_component_image) - sizeof(iomsg_header);
    msg->layer_id = cdesc->layer_id;
    msg->image_width = image_width;
    msg->image_height = image_height;
    msg->pixel_format = pixel_format;
    msg->framebuf = framebuf;

    void* replymsgbuf = malloc(sizeof(iomsg_reply_create_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return NULL;
    }

    iomsg_reply_create_component* replymsg = (iomsg_reply_create_component*)replymsgbuf;

    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_create_component)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_CREATE_COMPONENT_IMAGE) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    component_descriptor* new_cdesc = (component_descriptor*)malloc(sizeof(component_descriptor));
    if (new_cdesc == NULL) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }
    new_cdesc->layer_id = replymsg->layer_id;

    free(msgbuf);
    free(replymsgbuf);
    return new_cdesc;
}

int create_layer(size_t x, size_t y, size_t width, size_t height, const void* framebuf, uint32_t flags) {
    iomsg_create_layer msg = {
        .header = {.cmd_id = IOMSG_CMD_CREATE_LAYER,
                   .payload_size = sizeof(iomsg_create_layer) - sizeof(iomsg_header)},
        .x = x,
        .y = y,
        .width = width,
        .height = height,
        .framebuf = framebuf,
        .flags = flags,
    };
    iomsg_reply_create_component reply;

    if (sys_iomsg(&msg, &reply, sizeof(reply)) == -1) {
        return -1;
    }

    if (reply.header.cmd_id != IOMSG_CMD_CREATE_LAYER) {
        return -1;
    }

    return reply.layer_id;
}

int move_layer(int layer_id, size_t x, size_t y) {
    iomsg_move_layer msg = {
        .header = {.cmd_id = IOMSG_CMD_MOVE_LAYER,
                   .payload_size = sizeof(iomsg_move_layer) - sizeof(iomsg_header)},
        .layer_id = layer_id,
        .x = x,
        .y = y,
    };
    iomsg_header reply;

    if (sys_iomsg(&msg, &reply, sizeof(reply)) == -1) {
        return -1;
    }

    return reply.cmd_id == IOMSG_CMD_MOVE_LAYER ? 0 : -1;
}

int remove_layer(int layer_id) {
    iomsg_remove_component msg = {
        .header = {.cmd_id = IOMSG_CMD_REMOVE_COMPONENT, .payload_size = sizeof(int)},
        .layer_id = layer_id,
    };
    iomsg_reply_remove_component reply;

    if (sys_iomsg(&msg, &reply, sizeof(reply)) == -1) {
        return -1;
    }

    return reply.header.cmd_id == IOMSG_CMD_REMOVE_COMPONENT ? 0 : -1;
}

int get_screen_size(size_t* width, size_t* height) {
    iomsg_get_screen_size msg = {
        .header = {.cmd_id = IOMSG_CMD_GET_SCREEN_SIZE, .payload_size = 0},
    };
    iomsg_reply_get_screen_size reply;

    if (sys_iomsg(&msg, &reply, sizeof(reply)) == -1) {
        return -1;
    }

    if (reply.header.cmd_id != IOMSG_CMD_GET_SCREEN_SIZE) {
        return -1;
    }

    *width = reply.width;
    *height = reply.height;
    return 0;
}

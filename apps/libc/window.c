#include "window.h"

#include "stdio.h"
#include "stdlib.h"
#include "string.h"
#include "syscalls.h"

WindowDescriptor *create_window(const char *title, size_t x_pos, size_t y_pos, size_t width, size_t height) {
    size_t title_len = strlen(title) + 1;
    void *msgbuf = malloc(sizeof(iomsg_create_window) + title_len);
    if (msgbuf == NULL) {
        return NULL;
    }

    iomsg_create_window *msg = (iomsg_create_window *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_WINDOW;
    msg->header.payload_size = 8 * 4 + title_len;
    msg->x_pos = x_pos;
    msg->y_pos = y_pos;
    msg->width = width;
    msg->height = height;
    memcpy(msg->title, title, title_len);

    void *replymsgbuf = malloc(sizeof(iomsg_reply_create_window));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return NULL;
    }

    iomsg_reply_create_window *replymsg = (iomsg_reply_create_window *)replymsgbuf;

    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_create_window)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_CREATE_WINDOW) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    WindowDescriptor *wdesc = (WindowDescriptor *)malloc(sizeof(WindowDescriptor));
    if (wdesc == NULL) {
        free(msgbuf);
        free(replymsgbuf);
        return NULL;
    }

    wdesc->layer_id = replymsg->layer_id;

    free(msgbuf);
    free(replymsgbuf);
    return wdesc;
}

int destroy_window(WindowDescriptor *wdesc) {
    if (wdesc == NULL) {
        return -1;
    }

    void *msgbuf = malloc(sizeof(iomsg_destroy_window));
    if (msgbuf == NULL) {
        return -1;
    }

    iomsg_destroy_window *msg = (iomsg_destroy_window *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_DESTROY_WINDOW;
    msg->header.payload_size = sizeof(iomsg_destroy_window) - sizeof(iomsg_header);
    msg->layer_id = wdesc->layer_id;

    void *replymsgbuf = malloc(sizeof(iomsg_reply_destroy_window));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return -1;
    }

    iomsg_reply_destroy_window *replymsg = (iomsg_reply_destroy_window *)replymsgbuf;
    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_destroy_window)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_DESTROY_WINDOW) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    free(msgbuf);
    free(replymsgbuf);
    free(wdesc);
    return 0;
}

int add_image_to_window(WindowDescriptor *wdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void *framebuf) {
    if (wdesc == NULL || framebuf == NULL) {
        return -1;
    }

    void *msgbuf = malloc(sizeof(iomsg_add_image_to_window));
    if (msgbuf == NULL) {
        return -1;
    }

    iomsg_add_image_to_window *msg = (iomsg_add_image_to_window *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_ADD_IMAGE_TO_WINDOW;
    msg->header.payload_size = 40;  // FIXME
    msg->layer_id = wdesc->layer_id;
    msg->image_width = image_width;
    msg->image_height = image_height;
    msg->pixel_format = pixel_format;
    msg->framebuf = framebuf;

    void *replymsgbuf = malloc(sizeof(iomsg_reply_add_image_to_window));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return -1;
    }

    iomsg_reply_add_image_to_window *replymsg = (iomsg_reply_add_image_to_window *)replymsgbuf;

    if (sys_iomsg(msgbuf, replymsgbuf, sizeof(iomsg_reply_add_image_to_window)) == -1) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    if (replymsg->header.cmd_id != IOMSG_CMD_ADD_IMAGE_TO_WINDOW) {
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    free(msgbuf);
    free(replymsgbuf);
    return 0;
}

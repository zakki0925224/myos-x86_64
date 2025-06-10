#include "window.h"

#include "stdio.h"
#include "stdlib.h"
#include "string.h"
#include "syscalls.h"

int remove_component(ComponentDescriptor *cdesc) {
    if (cdesc == NULL) {
        return -1;
    }

    void *msgbuf = malloc(sizeof(iomsg_remove_component));
    if (msgbuf == NULL) {
        return -1;
    }

    iomsg_remove_component *msg = (iomsg_remove_component *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_REMOVE_COMPONENT;
    msg->header.payload_size = sizeof(iomsg_remove_component) - sizeof(iomsg_header);
    msg->layer_id = cdesc->layer_id;

    void *replymsgbuf = malloc(sizeof(iomsg_reply_remove_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return -1;
    }

    iomsg_reply_remove_component *replymsg = (iomsg_reply_remove_component *)replymsgbuf;
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

ComponentDescriptor *create_component_window(const char *title, size_t x_pos, size_t y_pos, size_t width, size_t height) {
    size_t title_len = strlen(title) + 1;
    void *msgbuf = malloc(sizeof(iomsg_create_component_window) + title_len);
    if (msgbuf == NULL) {
        return NULL;
    }

    iomsg_create_component_window *msg = (iomsg_create_component_window *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_COMPONENT_WINDOW;
    msg->header.payload_size = 8 * 4 + title_len;
    msg->x_pos = x_pos;
    msg->y_pos = y_pos;
    msg->width = width;
    msg->height = height;
    memcpy(msg->title, title, title_len);

    void *replymsgbuf = malloc(sizeof(iomsg_reply_create_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return NULL;
    }

    iomsg_reply_create_component *replymsg = (iomsg_reply_create_component *)replymsgbuf;

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

    ComponentDescriptor *cdesc = (ComponentDescriptor *)malloc(sizeof(ComponentDescriptor));
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

ComponentDescriptor *create_component_image(ComponentDescriptor *cdesc, size_t image_width, size_t image_height, uint8_t pixel_format, const void *framebuf) {
    if (cdesc == NULL || framebuf == NULL) {
        return NULL;
    }

    void *msgbuf = malloc(sizeof(iomsg_create_component_image));
    if (msgbuf == NULL) {
        return NULL;
    }

    iomsg_create_component_image *msg = (iomsg_create_component_image *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_COMPONENT_IMAGE;
    msg->header.payload_size = 40;  // FIXME
    msg->layer_id = cdesc->layer_id;
    msg->image_width = image_width;
    msg->image_height = image_height;
    msg->pixel_format = pixel_format;
    msg->framebuf = framebuf;

    void *replymsgbuf = malloc(sizeof(iomsg_reply_create_component));
    if (replymsgbuf == NULL) {
        free(msgbuf);
        return NULL;
    }

    iomsg_reply_create_component *replymsg = (iomsg_reply_create_component *)replymsgbuf;

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

    ComponentDescriptor *new_cdesc = (ComponentDescriptor *)malloc(sizeof(ComponentDescriptor));
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

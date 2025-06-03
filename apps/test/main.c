#include <stdio.h>
#include <stdlib.h>
#include <syscalls.h>

int main(int argc, char *argv[]) {
    void *msgbuf = malloc(4096);
    if (msgbuf == NULL) {
        printf("allocation error\n");
        return -1;
    }

    iomsg_create_window *msg = (iomsg_create_window *)msgbuf;
    msg->header.cmd_id = IOMSG_CMD_CREATE_WINDOW;
    msg->header.payload_size = 8 * 4 + 12;
    msg->x_pos = 200;
    msg->y_pos = 50;
    msg->width = 300;
    msg->height = 200;
    snprintf(msg->title, 12, "Test Window");

    void *replymsgbuf = malloc(4096);
    if (replymsgbuf == NULL) {
        printf("allocation error\n");
        free(msgbuf);
        return -1;
    }
    iomsg_reply_create_window *replymsg = (iomsg_reply_create_window *)replymsgbuf;

    if (sys_iomsg(msgbuf, replymsgbuf, 4096) == -1) {
        printf("sys_iomsg failed\n");
        free(msgbuf);
        free(replymsgbuf);
        return -1;
    }

    printf("sys_iomsg succeeded\n");
    printf("window id: %d\n", replymsg->layer_id);
    free(msgbuf);
    free(replymsgbuf);

    for (;;) {
    }

    return 0;
}

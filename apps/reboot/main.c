#include <stdio.h>
#include <string.h>
#include <syscalls.h>

#define POWER_DEV_PATH "/dev/power"
#define POWER_CMD "reboot"

int main(int argc, char* argv[]) {
    int fd = sys_open(POWER_DEV_PATH, OPEN_FLAG_NONE);
    if (fd == -1) {
        printf("reboot: failed to open %s\n", POWER_DEV_PATH);
        return -1;
    }

    if (sys_write(fd, POWER_CMD, strlen(POWER_CMD)) == -1) {
        printf("reboot: failed to write to %s\n", POWER_DEV_PATH);
        sys_close(fd);
        return -1;
    }

    sys_close(fd); // unreachable on success
    return 0;
}

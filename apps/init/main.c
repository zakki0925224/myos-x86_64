#include <stdio.h>
#include <syscalls.h>

int main(int argc, char* argv[]) {
    pid_t pid = sys_exec("/mnt/initramfs/apps/bin/sh /mnt/initramfs/apps/bin", EXEC_PIPE_NONE);
    if (pid == -1) {
        printf("init: sys_exec failed\n");
        return -1;
    }

    int exit_code = sys_wait(pid);
    printf("init: sh exited with %d\n", exit_code);

    while (1) {
    }

    return 0;
}

#include <stdio.h>
#include <syscalls.h>

int main(int argc, char* argv[]) {
    if (argc < 2) {
        return 0;
    }

    if (sys_chdir(argv[1]) == -1) {
        printf("cd: failed to change directory\n");
        return -1;
    }

    return 0;
}

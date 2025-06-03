#include <stdio.h>
#include <syscalls.h>

static char cwdenames[1280] = {0};

int main(int argc, char *argv[]) {
    char *path;

    if (argc > 1) {
        path = argv[1];
    } else {
        path = ".";
    }

    if (sys_getenames(path, cwdenames, sizeof(cwdenames)) == -1) {
        printf("ls: failed to get entry names\n");
        return -1;
    }

    char old_c = '\0';

    for (int i = 0; i < (int)sizeof(cwdenames); i++) {
        char c = cwdenames[i];

        // end of name list
        if (old_c == '\0' && c == '\0' && i > 0) {
            break;
        }

        if (c == '\0') {
            printf("  ");
        } else {
            printf("%c", c);
        }

        old_c = cwdenames[i];

        // clear
        cwdenames[i] = '\0';
    }
    printf("\n");

    return 0;
}

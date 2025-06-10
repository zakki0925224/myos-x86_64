#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>
#include <window.h>

#define BUF_LEN 128

static char buf[BUF_LEN] = {0};
static char *splitted_buf[BUF_LEN];
static char cwd_path[BUF_LEN] = {0};
static char envpath[BUF_LEN] = {0};
static char filename_buf[BUF_LEN] = {0};

void exec_cmd(char *cmd) {
    int cmdargs_len = split(cmd, ' ', splitted_buf, BUF_LEN);

    if (cmdargs_len < 1) {
        return;
    }

    if (strlen(splitted_buf[0]) == 0) {
        return;
    }

    if (strcmp(splitted_buf[0], "help") == 0) {
        printf("sh: Built-in commands:\n");
        printf("  help\n");
        printf("  exit\n");
        printf("  break\n");
        printf("  exec\n");
        printf("  window\n");
        printf("  clear\n");

        if (strlen(envpath) > 0) {
            printf("sh: envpath available\n");
            printf("  <COMMAND> is alias for \"exec %s/<COMMAND>\"\n", envpath);
        }
    } else if (strcmp(splitted_buf[0], "exit") == 0) {
        exit(0);
    } else if (strcmp(splitted_buf[0], "break") == 0) {
        sys_break();
        // __asm__ volatile("int3");
    } else if (strcmp(splitted_buf[0], "exec") == 0) {
        if (cmdargs_len < 2) {
            printf("sh: exec: missing argument\n");
            return;
        }

        char *args = splitted_buf[1];
        if (cmdargs_len > 2) {
            args = concatenate((const char **)(splitted_buf + 1), cmdargs_len - 1, " ");

            if (args == NULL) {
                printf("sh: exec: failed to concatenate arguments\n");
                return;
            }
        }

        if (sys_exec(args, EXEC_FLAG_DEBUG) == -1) {
            printf("sh: exec: failed to execute\n");
            return;
        }
    } else if (strcmp(splitted_buf[0], "window") == 0) {
        ComponentDescriptor *cdesc = create_component_window("test window", 200, 50, 300, 200);
        if (cdesc == NULL) {
            printf("sh: window: failed to create window\n");
            return;
        }
    } else if (strcmp(splitted_buf[0], "clear") == 0) {
        printf("\e[2J");
        printf("\e[1;1H");
    }
    // execute command with envpath
    else if (strlen(envpath) > 0) {
        snprintf(filename_buf, sizeof(filename_buf), "%s/%s", envpath, splitted_buf[0]);
        splitted_buf[0] = filename_buf;
        char *args = splitted_buf[0];
        if (cmdargs_len > 1) {
            args = concatenate((const char **)splitted_buf, cmdargs_len, " ");

            if (args == NULL) {
                printf("sh: exec: failed to concatenate arguments\n");
                return;
            }
        }

        if (sys_exec(args, EXEC_FLAG_NONE) == -1) {
            printf("sh: exec: failed to execute\n");
            return;
        }
    }
    // unreachable
    else {
        printf("sh: %s: command not found\n", cmd);
    }
}

int main(int argc, char const *argv[]) {
    int getcwd_ret;

    if (argc > 1) {
        strncpy(envpath, argv[1], strlen(argv[1]));
        printf("sh: set envpath: %s\n", envpath);
    }

    while (1) {
        getcwd_ret = sys_getcwd(cwd_path, sizeof(cwd_path));
        printf("\n\e[34m[%s]\e[m$ ", getcwd_ret == -1 ? "UNKNOWN" : cwd_path);

        if (sys_read(0, buf, BUF_LEN) == -1) {
            printf("Failed to read stdin\n");
            return -1;
        }

        replace(buf, '\n', '\0');
        exec_cmd(buf);
    }

    return 0;
}

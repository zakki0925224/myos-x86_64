#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>

#define BUF_LEN 128

static char buf[BUF_LEN] = {0};
static char* splitted_buf[BUF_LEN];
static char cwd_path[BUF_LEN] = {0};
static char envpath[BUF_LEN] = {0};
static char filepath_buf[BUF_LEN] = {0};

static int sh_readline(char* dst, int dst_len) {
    int len = 0;
    char tmp[BUF_LEN];

    while (1) {
        int ret = sys_read(0, tmp, sizeof(tmp));
        if (ret <= 0) return -1;

        for (int i = 0; i < ret; i++) {
            if (tmp[i] == '\n') {
                dst[len] = '\0';
                return 0;
            }
            if (len < dst_len - 1) dst[len++] = tmp[i];
        }
    }
}

static int build_exec_args(const char* cmd_str, char* out, int out_len) {
    while (*cmd_str == ' ') cmd_str++;
    if (*cmd_str == '\0') return -1;

    if (strlen(envpath) > 0) {
        // find end of command name (first space or end of string)
        const char* name_end = cmd_str;
        while (*name_end != ' ' && *name_end != '\0') name_end++;

        // copy command name to temp buffer to avoid %.*s
        char name_buf[BUF_LEN] = {0};
        int name_len = name_end - cmd_str;
        strncpy(name_buf, cmd_str, name_len);

        if (*name_end == '\0') {
            snprintf(out, out_len, "%s/%s", envpath, name_buf);
        } else {
            // name_end points to ' ' followed by args
            snprintf(out, out_len, "%s/%s%s", envpath, name_buf, name_end);
        }
    } else {
        strncpy(out, cmd_str, out_len - 1);
        out[out_len - 1] = '\0';
    }
    return 0;
}

static int strip_background(const char* cmd, char* out, int out_len) {
    int len = strlen(cmd);
    while (len > 0 && cmd[len - 1] == ' ') len--;
    if (len == 0 || cmd[len - 1] != '&') return 0;

    len--;
    while (len > 0 && cmd[len - 1] == ' ') len--;
    if (len > out_len - 1) len = out_len - 1;

    strncpy(out, cmd, len);
    out[len] = '\0';
    return 1;
}

static void exec_background(const char* cmd) {
    static char args[BUF_LEN];

    if (strchr(cmd, '|') != NULL) {
        printf("sh: &: pipe is not supported\n");
        return;
    }

    if (build_exec_args(cmd, args, BUF_LEN) < 0) {
        printf("sh: &: invalid command\n");
        return;
    }

    pid_t pid = sys_exec(args, EXEC_PIPE_NONE);
    if (pid < 0) {
        printf("sh: exec: failed\n");
        return;
    }

    printf("[%d]\n", (int)pid);
}

void exec_cmd(char* cmd) {
    static char bg_cmd[BUF_LEN];
    if (strip_background(cmd, bg_cmd, BUF_LEN)) {
        exec_background(bg_cmd);
        return;
    }

    char* pipe_pos = strchr(cmd, '|');
    if (pipe_pos != NULL) {
        *pipe_pos = '\0';
        char* left = cmd;
        char* right = pipe_pos + 1;

        static char left_args[BUF_LEN];
        static char right_args[BUF_LEN];

        if (build_exec_args(left, left_args, BUF_LEN) < 0 ||
            build_exec_args(right, right_args, BUF_LEN) < 0) {
            printf("sh: pipe: invalid command\n");
            return;
        }

        int pipefd[2];
        if (sys_pipe(pipefd) < 0) {
            printf("sh: pipe: failed\n");
            return;
        }

        pid_t pid1 = sys_exec(left_args, (int[]){-1, pipefd[1], -1});
        pid_t pid2 = sys_exec(right_args, (int[]){pipefd[0], -1, -1});

        if (pid1 < 0 || pid2 < 0) {
            printf("sh: pipe: exec failed\n");
        }

        if (pid1 >= 0) sys_wait(pid1);
        sys_close(pipefd[1]);  // EOF for the reader
        if (pid2 >= 0) sys_wait(pid2);
        sys_close(pipefd[0]);
        return;
    }

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
        printf("  exec\n");
        printf("  clear\n");

        if (strlen(envpath) > 0) {
            printf("sh: envpath available\n");
            printf("  <COMMAND> is alias for \"exec %s/<COMMAND>\"\n", envpath);
        }
    } else if (strcmp(splitted_buf[0], "exit") == 0) {
        exit(0);
    } else if (strcmp(splitted_buf[0], "exec") == 0) {
        if (cmdargs_len < 2) {
            printf("sh: exec: missing argument\n");
            return;
        }

        char* args = splitted_buf[1];
        if (cmdargs_len > 2) {
            args = concatenate((const char**)(splitted_buf + 1), cmdargs_len - 1, " ");

            if (args == NULL) {
                printf("sh: exec: failed to concatenate arguments\n");
                return;
            }
        }

        int exit_code = system(args);
        if (exit_code == -1) {
            printf("sh: exec: failed\n");
            return;
        }
    } else if (strcmp(splitted_buf[0], "clear") == 0) {
        printf("\e[2J");
        printf("\e[1;1H");
    }
    // execute command with envpath
    else if (strlen(envpath) > 0) {
        snprintf(filepath_buf, sizeof(filepath_buf), "%s/%s", envpath, splitted_buf[0]);
        splitted_buf[0] = filepath_buf;
        char* args = splitted_buf[0];
        if (cmdargs_len > 1) {
            args = concatenate((const char**)splitted_buf, cmdargs_len, " ");

            if (args == NULL) {
                printf("sh: exec: failed to concatenate arguments\n");
                return;
            }
        }

        int exit_code = system(args);
        if (exit_code == -1) {
            printf("sh: exec: failed\n");
            return;
        }
        printf("sh: exit code: %d\n", exit_code);
    }
    // unreachable
    else {
        printf("sh: %s: command not found\n", cmd);
    }
}

int main(int argc, char const* argv[]) {
    int getcwd_ret;

    if (argc > 1) {
        strncpy(envpath, argv[1], strlen(argv[1]));
        printf("sh: set envpath: %s\n", envpath);
    }

    while (1) {
        getcwd_ret = sys_getcwd(cwd_path, sizeof(cwd_path));
        printf("\n\e[34m[%s]\e[m$ ", getcwd_ret == -1 ? "UNKNOWN" : cwd_path);

        if (sh_readline(buf, BUF_LEN) == -1) {
            printf("Failed to read stdin\n");
            return -1;
        }

        exec_cmd(buf);
    }

    return 0;
}

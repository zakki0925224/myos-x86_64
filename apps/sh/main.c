#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>
#include <window.h>

#define BUF_LEN 128
#define HISTORY_MAX 16

static char buf[BUF_LEN] = {0};
static char* splitted_buf[BUF_LEN];
static char cwd_path[BUF_LEN] = {0};
static char envpath[BUF_LEN] = {0};
static char filepath_buf[BUF_LEN] = {0};

static char history[HISTORY_MAX][BUF_LEN];
static int hist_count = 0;

static void history_push(const char* line) {
    if (strlen(line) == 0) return;
    if (hist_count > 0 && strcmp(history[(hist_count - 1) % HISTORY_MAX], line) == 0) return;
    strncpy(history[hist_count % HISTORY_MAX], line, BUF_LEN - 1);
    history[hist_count % HISTORY_MAX][BUF_LEN - 1] = '\0';
    hist_count++;
}

static int sh_readline(char* dst, int dst_len) {
    int len = 0;
    int hist_pos = hist_count;
    char saved_line[BUF_LEN] = {0};

    while (1) {
        char c;
        if (sys_read(0, &c, 1) == -1) return -1;

        if (c == '\n') {
            dst[len] = '\0';
            break;
        } else if (c == '\x08' || c == '\x7f') {
            if (len > 0) {
                len--;
                dst[len] = '\0';
            }
            continue;
        } else if (c == '\x1b') { /* escape sequence */
            char c2, c3;
            if (sys_read(0, &c2, 1) == -1) return -1;
            if (c2 != '[') continue;
            if (sys_read(0, &c3, 1) == -1) return -1;
            c = (c3 == 'A') ? '\x10' : (c3 == 'B') ? '\x0e'
                                                   : '\0';
            if (c == '\0') continue;
        }

        if (c == '\x10') { /* cursor up: history prev */
            if (hist_count == 0) continue;
            if (hist_pos == hist_count)
                strncpy(saved_line, dst, BUF_LEN - 1);
            if (hist_pos > 0 &&
                (hist_count <= HISTORY_MAX || hist_pos > hist_count - HISTORY_MAX)) {
                hist_pos--;
                int old_len = len;
                strncpy(dst, history[hist_pos % HISTORY_MAX], dst_len - 1);
                dst[dst_len - 1] = '\0';
                len = strlen(dst);
                for (int i = 0; i < old_len; i++) sys_write(1, "\x08", 1);
                sys_write(1, dst, len);
                for (int i = len; i < old_len; i++) sys_write(1, " ", 1);
                for (int i = len; i < old_len; i++) sys_write(1, "\x08", 1);
            }
        } else if (c == '\x0e') { /* cursor down: history next */
            if (hist_pos >= hist_count) continue;
            int old_len = len;
            hist_pos++;
            if (hist_pos == hist_count)
                strncpy(dst, saved_line, dst_len - 1);
            else
                strncpy(dst, history[hist_pos % HISTORY_MAX], dst_len - 1);
            dst[dst_len - 1] = '\0';
            len = strlen(dst);
            for (int i = 0; i < old_len; i++) sys_write(1, "\x08", 1);
            sys_write(1, dst, len);
            for (int i = len; i < old_len; i++) sys_write(1, " ", 1);
            for (int i = len; i < old_len; i++) sys_write(1, "\x08", 1);
        } else {
            if (len < dst_len - 1) {
                dst[len++] = c;
                dst[len] = '\0';
            }
        }
    }
    return 0;
}

void exec_cmd(char* cmd) {
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

        char* args = splitted_buf[1];
        if (cmdargs_len > 2) {
            args = concatenate((const char**)(splitted_buf + 1), cmdargs_len - 1, " ");

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
        component_descriptor* cdesc = create_component_window("test window", 200, 50, 300, 200);
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
        history_push(buf);
    }

    return 0;
}

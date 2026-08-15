#include <ctype.h>
#include <lauxlib.h>
#include <lua.h>
#include <lualib.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>

#define BUF_LEN 128
#define HISTORY_MAX 16

static char buf[BUF_LEN] = {0};
static char envpath[BUF_LEN] = {0};

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

static char* trim(char* s) {
    while (*s == ' ' || *s == '\t') s++;
    char* end = s + strlen(s);
    while (end > s && (end[-1] == ' ' || end[-1] == '\t')) end--;
    *end = '\0';
    return s;
}

static void print_results(lua_State* L, int top_before) {
    int nres = lua_gettop(L) - top_before;
    if (nres > 0) {
        lua_getglobal(L, "print");
        lua_insert(L, top_before + 1);
        if (lua_pcall(L, nres, 0, 0) != LUA_OK) {
            printf("lush: %s\n", lua_tostring(L, -1));
        }
    }
    lua_settop(L, top_before);
}

static void eval_expr_and_print(lua_State* L, const char* expr) {
    char wrapped[BUF_LEN + 8];
    int top_before = lua_gettop(L);

    snprintf(wrapped, sizeof(wrapped), "return %s", expr);
    if (luaL_loadstring(L, wrapped) != LUA_OK) {
        printf("lush: %s\n", lua_tostring(L, -1));
        lua_settop(L, top_before);
        return;
    }

    if (lua_pcall(L, 0, LUA_MULTRET, 0) != LUA_OK) {
        printf("lush: %s\n", lua_tostring(L, -1));
        lua_settop(L, top_before);
        return;
    }

    print_results(L, top_before);
}

static int try_eval_lua_stmt(lua_State* L, const char* line) {
    int top_before = lua_gettop(L);

    if (luaL_loadstring(L, line) != LUA_OK) {
        lua_settop(L, top_before);
        return 0;
    }

    if (lua_pcall(L, 0, LUA_MULTRET, 0) != LUA_OK) {
        printf("lush: %s\n", lua_tostring(L, -1));
        lua_settop(L, top_before);
        return 1;
    }

    print_results(L, top_before);

    return 1;
}

static void take_leading_ident(const char* line, char* out, int out_len) {
    while (*line == ' ') line++;
    int i = 0;
    while ((isalnum((unsigned char)*line) || *line == '_') && i < out_len - 1) {
        out[i++] = *line++;
    }
    out[i] = '\0';
}

static int looks_like_assignment(const char* line) {
    for (const char* p = line; *p; p++) {
        if (*p == '(' || *p == '"' || *p == '\'' || *p == '{') return 0;
        if (*p == '=') {
            char prev = (p > line) ? p[-1] : '\0';
            char next = p[1];
            if (prev == '=' || prev == '~' || prev == '<' || prev == '>' || next == '=') continue;
            return 1;
        }
    }
    return 0;
}

static int is_known_lua_leader(lua_State* L, const char* line) {
    static const char* keywords[] = {
        "and", "break", "do", "else", "elseif", "end", "false", "for", "function",
        "goto", "if", "in", "local", "nil", "not", "or", "repeat", "return",
        "then", "true", "until", "while", NULL};

    char name[BUF_LEN];
    take_leading_ident(line, name, BUF_LEN);
    if (name[0] == '\0') return 1;

    for (int i = 0; keywords[i] != NULL; i++) {
        if (strcmp(name, keywords[i]) == 0) return 1;
    }

    lua_getglobal(L, name);
    int known = !lua_isnil(L, -1);
    lua_pop(L, 1);
    return known;
}

static void builtin_cd(const char* args) {
    while (*args == ' ') args++;
    const char* path = (*args == '\0') ? "/" : args;

    if (sys_chdir(path) == -1) {
        printf("lush: cd: %s: No such file or directory\n", path);
    }
}

static int build_exec_args(const char* cmd_str, char* out, int out_len) {
    while (*cmd_str == ' ') cmd_str++;
    if (*cmd_str == '\0') return -1;

    if (strlen(envpath) > 0) {
        const char* name_end = cmd_str;
        while (*name_end != ' ' && *name_end != '\0') name_end++;

        char name_buf[BUF_LEN] = {0};
        int name_len = name_end - cmd_str;
        strncpy(name_buf, cmd_str, name_len);

        if (*name_end == '\0') {
            snprintf(out, out_len, "%s/%s", envpath, name_buf);
        } else {
            snprintf(out, out_len, "%s/%s%s", envpath, name_buf, name_end);
        }
    } else {
        strncpy(out, cmd_str, out_len - 1);
        out[out_len - 1] = '\0';
    }
    return 0;
}

static char* find_unquoted_pipe(char* line) {
    int in_dquote = 0, in_squote = 0;
    for (char* p = line; *p; p++) {
        if (*p == '"' && !in_squote) {
            in_dquote = !in_dquote;
        } else if (*p == '\'' && !in_dquote) {
            in_squote = !in_squote;
        } else if (*p == '|' && !in_dquote && !in_squote) {
            return p;
        }
    }
    return NULL;
}

static void exec_pipe(char* cmd, char* pipe_pos) {
    *pipe_pos = '\0';
    char* left = cmd;
    char* right = pipe_pos + 1;

    static char left_args[BUF_LEN];
    static char right_args[BUF_LEN];

    if (build_exec_args(left, left_args, BUF_LEN) < 0 ||
        build_exec_args(right, right_args, BUF_LEN) < 0) {
        printf("lush: pipe: invalid command\n");
        return;
    }

    int pipefd[2];
    if (sys_pipe(pipefd) < 0) {
        printf("lush: pipe: failed\n");
        return;
    }

    pid_t pid1 = sys_exec(left_args, (int[]){-1, pipefd[1], -1});
    pid_t pid2 = sys_exec(right_args, (int[]){pipefd[0], -1, -1});

    if (pid1 < 0 || pid2 < 0) {
        printf("lush: pipe: exec failed\n");
        return;
    }

    sys_wait(pid1);
    sys_wait(pid2);
}

int main(int argc, char const* argv[]) {
    if (argc > 1) {
        strncpy(envpath, argv[1], BUF_LEN - 1);
        printf("lush: set envpath: %s\n", envpath);
    }

    lua_State* L = luaL_newstate();
    if (L == NULL) {
        printf("lush: luaL_newstate failed\n");
        return -1;
    }
    luaL_openlibs(L);

    char cwd_path[BUF_LEN];
    char resolved[BUF_LEN];

    while (1) {
        int getcwd_ret = sys_getcwd(cwd_path, sizeof(cwd_path));
        printf("\n\e[35m[%s]\e[m$ ", getcwd_ret == -1 ? "UNKNOWN" : cwd_path);

        if (sh_readline(buf, BUF_LEN) == -1) {
            printf("lush: failed to read stdin\n");
            break;
        }

        char* line = trim(buf);
        if (*line == '\0') continue;

        if (strcmp(line, "exit") == 0 || strcmp(line, "quit") == 0) {
            break;
        }

        if (strncmp(line, "cd", 2) == 0 && (line[2] == '\0' || line[2] == ' ')) {
            builtin_cd(line + 2);
            history_push(buf);
            continue;
        }

        char* pipe_pos = find_unquoted_pipe(line);
        if (pipe_pos != NULL) {
            exec_pipe(line, pipe_pos);
            history_push(buf);
            continue;
        }

        if (line[0] == '=') {
            eval_expr_and_print(L, line + 1);
            history_push(buf);
            continue;
        }

        int handled = 0;
        if (looks_like_assignment(line) || is_known_lua_leader(L, line)) {
            handled = try_eval_lua_stmt(L, line);
        }

        if (!handled) {
            if (build_exec_args(line, resolved, BUF_LEN) < 0) {
                history_push(buf);
                continue;
            }

            int exit_code = system(resolved);
            if (exit_code == -1) {
                printf("lush: %s: command not found\n", line);
            } else {
                printf("lush: exit code: %d\n", exit_code);
            }
        }

        history_push(buf);
    }

    lua_close(L);

    return 0;
}

#include <stdio.h>
#include <string.h>
#include <syscalls.h>

#define ENAMES_LEN 1280
#define STATUS_LEN 256
#define FIELD_LEN 64

static char enames[ENAMES_LEN] = {0};

static int is_numeric(const char* s) {
    if (*s == '\0') return 0;

    for (; *s != '\0'; s++) {
        if (*s < '0' || *s > '9') return 0;
    }

    return 1;
}

// find "<field>\t<value>\n" in status and copy <value> into out
static int read_field(const char* status, const char* field, char* out, size_t out_len) {
    const char* line = status;

    while (*line != '\0') {
        if (strncmp(line, field, strlen(field)) == 0) {
            const char* value = strchr(line, '\t');
            if (value == NULL) return -1;
            value++;

            size_t i = 0;
            while (value[i] != '\n' && value[i] != '\0' && i + 1 < out_len) {
                out[i] = value[i];
                i++;
            }
            out[i] = '\0';
            return 0;
        }

        const char* next = strchr(line, '\n');
        if (next == NULL) break;
        line = next + 1;
    }

    return -1;
}

static void print_padded(const char* s, int width) {
    printf("%s", s);
    for (int i = strlen(s); i < width; i++) printf(" ");
}

static int print_task(const char* pid) {
    char path[32];
    snprintf(path, sizeof(path), "/proc/%s/status", pid);

    FILE* file = fopen(path, "r");
    if (file == NULL) return -1;

    static char status[STATUS_LEN];
    memset(status, 0, sizeof(status));
    size_t n = fread(status, 1, sizeof(status) - 1, file);
    fclose(file);
    if (n == 0) return -1;

    char name[FIELD_LEN], ppid[FIELD_LEN], state[FIELD_LEN];
    if (read_field(status, "Name:", name, sizeof(name)) == -1) return -1;
    if (read_field(status, "PPid:", ppid, sizeof(ppid)) == -1) return -1;
    if (read_field(status, "State:", state, sizeof(state)) == -1) return -1;

    print_padded(pid, 6);
    print_padded(ppid, 6);
    print_padded(state, 10);
    printf("%s\n", name);
    return 0;
}

int main(int argc, char const* argv[]) {
    if (sys_getenames("/proc", enames, sizeof(enames)) == -1) {
        printf("ps: failed to get entry names of /proc\n");
        return -1;
    }

    print_padded("PID", 6);
    print_padded("PPID", 6);
    print_padded("STATE", 10);
    printf("NAME\n");

    // enames is a NUL-separated name list terminated by an empty name
    for (size_t i = 0; i < sizeof(enames) && enames[i] != '\0';) {
        const char* name = &enames[i];

        if (is_numeric(name)) {
            // ignore failures: the task may have exited since getenames
            print_task(name);
        }

        i += strlen(name) + 1;
    }

    return 0;
}

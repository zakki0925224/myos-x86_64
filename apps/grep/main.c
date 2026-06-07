#include <string.h>
#include <syscalls.h>

int main(int argc, char* argv[]) {
    if (argc < 2) return 1;
    const char* pattern = argv[1];

    char buf[1024];
    int n;

    do {
        n = sys_read(0, buf, sizeof(buf) - 1);
    } while (n == 0);

    if (n < 0) return 1;
    buf[n] = '\0';

    char* line = buf;
    char* end;
    while ((end = strchr(line, '\n')) != NULL) {
        *end = '\0';
        if (strstr(line, pattern)) {
            sys_write(1, line, strlen(line));
            sys_write(1, "\n", 1);
        }
        line = end + 1;
    }
    if (*line && strstr(line, pattern)) {
        sys_write(1, line, strlen(line));
        sys_write(1, "\n", 1);
    }

    return 0;
}

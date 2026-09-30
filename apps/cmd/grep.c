#include <stdio.h>
#include <string.h>

int main(int argc, char* argv[]) {
    if (argc < 2) return 1;
    const char* pattern = argv[1];

    char line[1024];
    while (fgets(line, sizeof(line), stdin) != NULL) {
        char* nl = strchr(line, '\n');
        if (nl != NULL) *nl = '\0';

        if (strstr(line, pattern)) {
            fputs(line, stdout);
            fputs("\n", stdout);
        }
    }

    return 0;
}

#include <stdio.h>
#include <string.h>

#define POWER_DEV_PATH "/dev/power"
#define POWER_CMD "reboot"

int main(int argc, char* argv[]) {
    FILE* file = fopen(POWER_DEV_PATH, "r+");
    if (file == NULL) {
        printf("reboot: failed to open %s\n", POWER_DEV_PATH);
        return -1;
    }

    if (fwrite(POWER_CMD, 1, strlen(POWER_CMD), file) != strlen(POWER_CMD)) {
        printf("reboot: failed to write to %s\n", POWER_DEV_PATH);
        fclose(file);
        return -1;
    }

    fclose(file); // unreachable on success
    return 0;
}

#include <stdio.h>
#include <stdlib.h>
#include <syscalls.h>

#define MS_IN_A_DAY (24 * 60 * 60 * 1000)
#define MS_IN_A_HOUR (60 * 60 * 1000)
#define MS_IN_A_MINUTE (60 * 1000)
#define MS_IN_A_SECOND 1000

int main(int argc, char* argv[]) {
    uint64_t uptime_ms = sys_uptime();
    uint64_t days = uptime_ms / MS_IN_A_DAY;
    uint64_t hours = (uptime_ms % MS_IN_A_DAY) / MS_IN_A_HOUR;
    uint64_t minutes = (uptime_ms % MS_IN_A_HOUR) / MS_IN_A_MINUTE;
    uint64_t seconds = (uptime_ms % MS_IN_A_MINUTE) / MS_IN_A_SECOND;
    uint64_t milliseconds = (uptime_ms % MS_IN_A_SECOND);

    printf("%d ms\n", uptime_ms);
    printf("%d days %d hours %d minutes %d seconds %d milliseconds\n", days, hours, minutes, seconds, milliseconds);

    return 0;
}

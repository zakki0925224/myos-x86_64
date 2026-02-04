#include "signal.h"

#include <stddef.h>

#include "stdio.h"

sighandler_t signal(int signum, sighandler_t handler) {
    printf("[DEBUG]signal called\n");
    return SIG_DFL;
}

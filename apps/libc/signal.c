#include "signal.h"

#include <stddef.h>

sighandler_t signal(int signum, sighandler_t handler) {
    return SIG_DFL;
}

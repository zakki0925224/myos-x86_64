#include "time.h"

time_t time(time_t* t) {
    if (t != NULL) {
        *t = 0;
    }
    return 0;
}

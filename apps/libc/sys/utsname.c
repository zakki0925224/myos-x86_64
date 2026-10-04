#include "utsname.h"

#include "../string.h"
#include "../syscalls.h"

int uname(utsname* buf) {
    memset(buf, 0, sizeof(utsname));  // kernel does not NUL-terminate
    return sys_uname(buf);
}

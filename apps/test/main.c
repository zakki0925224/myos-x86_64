#include <stdio.h>
#include <syscalls.h>

int main(int argc, const char* argv[]) {
    sys_recvfrom(12345, NULL, 67890, 98765, NULL, 43210);
    return 0;
}

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <syscalls.h>
#include <utsname.h>

int main(int argc, const char* argv[]) {
    utsname* buf = (utsname*)malloc(sizeof(utsname));
    if (buf == NULL) {
        return -1;
    }

    sys_uname(buf);

    if (argc == 1) {
        printf("%s", buf->sysname);
        return 0;
    }

    if (strcmp(argv[1], "--help") == 0) {
        printf("Usage: uname [OPTION]...\n");
        printf("Print certain system information. With no OPTION, same as -s.\n\n");
        printf(" -a\tprint all information\n");
        printf(" -s\tprint the kernel name\n");
        printf(" -n\tprint the network node hostname\n");
        printf(" -r\tprint the kernel release\n");
        printf(" -v\tprint the kernel version\n");
        printf(" -m\tprint the machine hardware name\n");
        printf(" -d\tprint the domain name\n");
        return 0;
    }

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-a") == 0) {
            printf("%s %s %s %s %s %s", buf->sysname, buf->nodename, buf->release, buf->version, buf->machine, buf->domainname);
            return 0;
        }
    }

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "-s") == 0) {
            printf("%s ", buf->sysname);
        } else if (strcmp(argv[i], "-n") == 0) {
            printf("%s ", buf->nodename);
        } else if (strcmp(argv[i], "-r") == 0) {
            printf("%s ", buf->release);
        } else if (strcmp(argv[i], "-v") == 0) {
            printf("%s ", buf->version);
        } else if (strcmp(argv[i], "-m") == 0) {
            printf("%s ", buf->machine);
        } else if (strcmp(argv[i], "-d") == 0) {
            printf("%s ", buf->domainname);
        }
    }

    return 0;
}

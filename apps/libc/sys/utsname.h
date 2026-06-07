#ifndef _SYS_UTSNAME_H
#define _SYS_UTSNAME_H

#define UTS_LEN 64

typedef struct
{
    char sysname[UTS_LEN];
    char nodename[UTS_LEN];
    char release[UTS_LEN];
    char version[UTS_LEN];
    char machine[UTS_LEN];
    char domainname[UTS_LEN];
} utsname;

#endif

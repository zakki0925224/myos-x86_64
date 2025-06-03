#include <stddef.h>
#include <stdint.h>

#include "iomsg.h"
#include "stat.h"
#include "utsname.h"

#ifndef _SYSCALLS_H
#define _SYSCALLS_H

// syscall numbers
#define SN_READ 0
#define SN_WRITE 1
#define SN_OPEN 2
#define SN_CLOSE 3
#define SN_EXIT 4
#define SN_SBRK 5
#define SN_UNAME 6
#define SN_BREAK 7
#define SN_STAT 8
#define SN_UPTIME 9
#define SN_EXEC 10
#define SN_GETCWD 11
#define SN_CHDIR 12
// 13
// 14
#define SN_SBRKSZ 15
// 16
#define SN_GETENAMES 17
#define SN_IOMSG 18

// defined file descriptor numbers
#define FDN_STDIN 0
#define FDN_STDOUT 1
#define FDN_STDERR 2

// sys_open flags
#define OPEN_FLAG_NONE 0x0
#define OPEN_FLAG_CREATE 0x1

// sys_exec flags
#define EXEC_FLAG_NONE 0x0
#define EXEC_FLAG_DEBUG 0x1

extern int sys_read(int fd, void *buf, size_t buf_len);
extern int sys_write(int fd, const void *buf, size_t buf_len);
extern int sys_open(const char *filepath, uint32_t flags);
extern int sys_close(int fd);
extern void sys_exit(int status);
extern void *sys_sbrk(size_t len);
extern int sys_uname(utsname *buf);
extern void sys_break(void);
extern int sys_stat(int fd, f_stat *buf);
extern uint64_t sys_uptime(void);
extern int sys_exec(const char *args, uint32_t flags);
extern int sys_getcwd(char *buf, size_t buf_len);
extern int sys_chdir(const char *path);
extern size_t sys_sbrksz(const void *target);
extern int sys_getenames(const char *path, char *buf, size_t buf_len);
extern int sys_iomsg(const void *msgbuf, void *replymsgbuf, size_t replymsgbuf_len);

#endif

#ifndef _SYSCALLS_H
#define _SYSCALLS_H

#include <stddef.h>
#include <stdint.h>

#include "iomsg.h"
#include "stat.h"
#include "sys/socket.h"
#include "utsname.h"

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
#define SN_FREE 13
// 14
#define SN_SBRKSZ 15
// 16
#define SN_GETENAMES 17
#define SN_IOMSG 18
#define SN_SOCKET 19
#define SN_BIND 20
#define SN_SENDTO 21
#define SN_RECVFROM 22
#define SN_SEND 23
#define SN_RECV 24
#define SN_CONNECT 25
#define SN_LISTEN 26
#define SN_ACCEPT 27

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

// sys_socket args
#define SOCKET_DOMAIN_AF_INET 1
#define SOCKET_TYPE_SOCK_DGRAM 1
#define SOCKET_TYPE_SOCK_STREAM 2
#define SOCKET_PROTO_UDP 17

int sys_read(int fd, void* buf, size_t buf_len);
int sys_write(int fd, const void* buf, size_t buf_len);
int sys_open(const char* filepath, int flags);
int sys_close(int fd);
void sys_exit(int status);
void* sys_sbrk(size_t len);
int sys_uname(utsname* buf);
void sys_break(void);
int sys_stat(int fd, f_stat* buf);
uint64_t sys_uptime(void);
int sys_exec(const char* args, int flags);
int sys_getcwd(char* buf, size_t buf_len);
int sys_chdir(const char* path);
int sys_free(void* ptr);
size_t sys_sbrksz(const void* target);
int sys_getenames(const char* path, char* buf, size_t buf_len);
int sys_iomsg(const void* msgbuf, void* replymsgbuf, size_t replymsgbuf_len);
int sys_socket(int domain, int type, int protocol);
int sys_bind(int sockfd, const struct sockaddr* addr, size_t addrlen);
int sys_sendto(int sockfd, const void* buf, size_t len, int flags, const struct sockaddr* dest_addr, size_t addrlen);
int sys_recvfrom(int sockfd, void* buf, size_t len, int flags, struct sockaddr* src_addr, size_t addrlen);
int sys_send(int sockfd, const void* buf, size_t len, int flags);
int sys_recv(int sockfd, void* buf, size_t len, int flags);
int sys_connect(int sockfd, const struct sockaddr* addr, size_t addrlen);
int sys_listen(int sockfd, int backlog);
int sys_accept(int sockfd, struct sockaddr* addr, size_t* addrlen);

#endif

#include "syscalls.h"

static uint64_t syscall(uint64_t syscall_number, uint64_t arg1, uint64_t arg2, uint64_t arg3, uint64_t arg4, uint64_t arg5) {
    uint64_t ret_val;
    __asm__ volatile(
        "movq %1, %%rdi\n"
        "movq %2, %%rsi\n"
        "movq %3, %%rdx\n"
        "movq %4, %%r10\n"
        "movq %5, %%r8\n"
        "movq %6, %%r9\n"
        "syscall\n"
        "movq %%rax, %0\n"
        : "=r"(ret_val)
        : "r"(syscall_number), "r"(arg1), "r"(arg2), "r"(arg3), "r"(arg4), "r"(arg5)
        : "rdi", "rsi", "rdx", "r10", "r8", "r9", "memory");
    return ret_val;
}

int sys_read(int fd, void *buf, size_t buf_len) {
    return (int)syscall(SN_READ, (uint64_t)fd, (uint64_t)buf, (uint64_t)buf_len, 0, 0);
}

int sys_write(int fd, const void *buf, size_t buf_len) {
    return (int)syscall(SN_WRITE, (uint64_t)fd, (uint64_t)buf, (uint64_t)buf_len, 0, 0);
}

int sys_open(const char *filepath, uint32_t flags) {
    return (int)syscall(SN_OPEN, (uint64_t)filepath, (uint64_t)flags, 0, 0, 0);
}

int sys_close(int fd) {
    return (int)syscall(SN_CLOSE, (uint64_t)fd, 0, 0, 0, 0);
}

void sys_exit(int status) {
    syscall(SN_EXIT, (uint64_t)status, 0, 0, 0, 0);
}

void *sys_sbrk(size_t len) {
    uint64_t addr = syscall(SN_SBRK, (uint64_t)len, 0, 0, 0, 0);
    return (void *)addr;
}

int sys_uname(utsname *buf) {
    return (int)syscall(SN_UNAME, (uint64_t)buf, 0, 0, 0, 0);
}

void sys_break(void) {
    syscall(SN_BREAK, 0, 0, 0, 0, 0);
}

int sys_stat(int fd, f_stat *buf) {
    return (int)syscall(SN_STAT, (uint64_t)fd, (uint64_t)buf, 0, 0, 0);
}

uint64_t sys_uptime(void) {
    return syscall(SN_UPTIME, 0, 0, 0, 0, 0);
}

int sys_exec(const char *args, uint32_t flags) {
    return (int)syscall(SN_EXEC, (uint64_t)args, (uint64_t)flags, 0, 0, 0);
}

int sys_getcwd(char *buf, size_t buf_len) {
    return (int)syscall(SN_GETCWD, (uint64_t)buf, (uint64_t)buf_len, 0, 0, 0);
}

int sys_chdir(const char *path) {
    return (int)syscall(SN_CHDIR, (uint64_t)path, 0, 0, 0, 0);
}

size_t sys_sbrksz(const void *target) {
    return (size_t)syscall(SN_SBRKSZ, (uint64_t)target, 0, 0, 0, 0);
}

int sys_getenames(const char *path, char *buf, size_t buf_len) {
    return (int)syscall(SN_GETENAMES, (uint64_t)path, (uint64_t)buf, (uint64_t)buf_len, 0, 0);
}

int sys_iomsg(const void *msgbuf, void *replymsgbuf, size_t replymsgbuf_len) {
    return (int)syscall(SN_IOMSG, (uint64_t)msgbuf, (uint64_t)replymsgbuf, (uint64_t)replymsgbuf_len, 0, 0);
}

#include "syscalls.h"

static int64_t syscall(uint64_t syscall_number, uint64_t arg1, uint64_t arg2, uint64_t arg3, uint64_t arg4, uint64_t arg5) {
    int64_t ret_val;
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

int64_t sys_read(int64_t fd, void *buf, int buf_len) {
    return syscall(SN_READ, (uint64_t)fd, (uint64_t)buf, (uint64_t)buf_len, 0, 0);
}

int64_t sys_write(int64_t fd, const char *str, int len) {
    return syscall(SN_WRITE, (uint64_t)fd, (uint64_t)str, (uint64_t)len, 0, 0);
}

int64_t sys_open(const char *filepath, char flags) {
    return syscall(SN_OPEN, (uint64_t)filepath, (uint64_t)flags, 0, 0, 0);
}

int64_t sys_close(int64_t fd) {
    return syscall(SN_CLOSE, (uint64_t)fd, 0, 0, 0, 0);
}

void sys_exit(uint64_t status) {
    syscall(SN_EXIT, status, 0, 0, 0, 0);
}

void *sys_sbrk(uint64_t len) {
    int64_t addr = syscall(SN_SBRK, len, 0, 0, 0, 0);
    return (void *)addr;
}

int64_t sys_uname(utsname *buf) {
    return syscall(SN_UNAME, (uint64_t)buf, 0, 0, 0, 0);
}

void sys_break() {
    syscall(SN_BREAK, 0, 0, 0, 0, 0);
}

int64_t sys_stat(int64_t fd, f_stat *buf) {
    return syscall(SN_STAT, (uint64_t)fd, (uint64_t)buf, 0, 0, 0);
}

uint64_t sys_uptime() {
    return (uint64_t)syscall(SN_UPTIME, 0, 0, 0, 0, 0);
}

int64_t sys_exec(const char *args, uint64_t flags) {
    return syscall(SN_EXEC, (uint64_t)args, flags, 0, 0, 0);
}

int64_t sys_getcwd(char *buf, int buf_len) {
    return syscall(SN_GETCWD, (uint64_t)buf, (uint64_t)buf_len, 0, 0, 0);
}

int64_t sys_chdir(const char *path) {
    return syscall(SN_CHDIR, (uint64_t)path, 0, 0, 0, 0);
}

int64_t sys_create_window(const char *title, uint64_t x_pos, uint64_t y_pos, uint64_t width, uint64_t height) {
    return syscall(SN_CREATE_WINDOW, (uint64_t)title, x_pos, y_pos, width, height);
}

int64_t sys_destroy_window(int64_t wd) {
    return syscall(SN_DESTROY_WINDOW, (uint64_t)wd, 0, 0, 0, 0);
}

size_t sys_sbrksz(const void *target) {
    return (size_t)syscall(SN_SBRKSZ, (uint64_t)target, 0, 0, 0, 0);
}

int64_t sys_add_image_to_window(int64_t wd, uint64_t image_width, uint64_t image_height, uint8_t pixel_format, const char *framebuf) {
    return syscall(SN_ADD_IMAGE_TO_WINDOW, (uint64_t)wd, image_width, image_height, (uint64_t)pixel_format, (uint64_t)framebuf);
}

int64_t sys_getenames(const char *path, char *buf, int buf_len) {
    return syscall(SN_GETENAMES, (uint64_t)path, (uint64_t)buf, (uint64_t)buf_len, 0, 0);
}

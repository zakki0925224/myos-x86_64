# libc

Standard C Library for MyOS

## Syscalls

### read

Reads from a file.

### write

Writes to a file.

### open

Opens a file.

### close

Closes a file.

### exit

Exits the application with a status (noreturn).

### sbrk

Allocates memory, aligned to 4KB.

### uname

Retrieves system information.

### break

Triggers a trap at the current instruction (noreturn).

### stat

Gets file information.

### uptime

Returns the system uptime in milliseconds.

### exec

Executes an ELF file.

### getcwd

Gets the absolute path of the current working directory.

### chdir

Changes the current working directory.

### sbrksz

Get the size of memory acquired by sbrk.

### getenames

Retrieves a list of entry names in a directory, separated by null characters (\0).

### iomsg

Sends a generic I/O message to the system for various advanced operations.
The message buffer should be formatted according to the specific command.
A reply buffer can be provided to receive the result or response from the system.

## Syscall tables

| number | name          | syscall num | arg1(%rdi)            | arg2(%rsi)         | arg3(%rdx)             | arg4(%r10) | arg5(%r8) | arg6(%r9) | ret(%rax)                        |
| ------ | ------------- | ----------- | --------------------- | ------------------ | ---------------------- | ---------- | --------- | --------- | -------------------------------- |
| 0      | sys_read      | 0x00        | int fd                | void \*buf         | size_t buf_len         | -          | -         | -         | int (read bytes, -1 on error)    |
| 1      | sys_write     | 0x01        | int fd                | const void \*buf   | size_t buf_len         | -          | -         | -         | int (written bytes, -1 on error) |
| 2      | sys_open      | 0x02        | const char \*filepath | uint32_t flags     | -                      | -          | -         | -         | int (fd, -1 on error)            |
| 3      | sys_close     | 0x03        | int fd                | -                  | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 4      | sys_exit      | 0x04        | int status            | -                  | -                      | -          | -         | -         | void (noreturn)                  |
| 5      | sys_sbrk      | 0x05        | size_t len            | -                  | -                      | -          | -         | -         | void\* (pointer, NULL on error)  |
| 6      | sys_uname     | 0x06        | struct utsname \*buf  | -                  | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 7      | sys_break     | 0x07        | -                     | -                  | -                      | -          | -         | -         | void (noreturn)                  |
| 8      | sys_stat      | 0x08        | int fd                | struct stat \*buf  | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 9      | sys_uptime    | 0x09        | -                     | -                  | -                      | -          | -         | -         | uint64_t (uptime ms)             |
| 10     | sys_exec      | 0x0a        | const char \*args     | uint32_t flags     | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 11     | sys_getcwd    | 0x0b        | char \*buf            | size_t buf_len     | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 12     | sys_chdir     | 0x0c        | const char \*path     | -                  | -                      | -          | -         | -         | int (0 on success, -1 on error)  |
| 13     | -             | -           | -                     | -                  | -                      | -          | -         | -         | -                                |
| 14     | -             | -           | -                     | -                  | -                      | -          | -         | -         | -                                |
| 15     | sys_sbrksz    | 0x0f        | const void \*target   | -                  | -                      | -          | -         | -         | size_t (size, 0 on error)        |
| 16     | -             | -           | -                     | -                  | -                      | -          | -         | -         | -                                |
| 17     | sys_getenames | 0x11        | const char \*path     | char \*buf         | size_t buf_len         | -          | -         | -         | int (0 on success, -1 on error)  |
| 18     | sys_iomsg     | 0x12        | const void \*msgbuf   | void \*replymsgbuf | size_t replymsgbuf_len | -          | -         | -         | int (0 on success, -1 on error)  |

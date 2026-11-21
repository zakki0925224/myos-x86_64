# libc

Standard C Library for MyOS

## Syscalls

| number | name          | description                                               | syscall num(%rax) | arg1(%rdi)            | arg2(%rsi)                   | arg3(%rdx)             | arg4(%r10) | arg5(%r8)                         | arg6(%r9)      | ret(%rax)                                  |
| ------ | ------------- | --------------------------------------------------------- | ----------------- | --------------------- | ---------------------------- | ---------------------- | ---------- | --------------------------------- | -------------- | ------------------------------------------ |
| 0      | sys_read      | Reads from a file.                                        | 0x00              | int fd                | void \*buf                   | size_t buf_len         | -          | -                                 | -              | int (read bytes, -1 on error)              |
| 1      | sys_write     | Writes to a file.                                         | 0x01              | int fd                | const void \*buf             | size_t buf_len         | -          | -                                 | -              | int (written bytes, -1 on error)           |
| 2      | sys_open      | Opens a file.                                             | 0x02              | const char \*filepath | int flags                    | -                      | -          | -                                 | -              | int (fd, -1 on error)                      |
| 3      | sys_close     | Closes a file.                                            | 0x03              | int fd                | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 4      | sys_exit      | Exits the application with a status (noreturn).           | 0x04              | int status            | -                            | -                      | -          | -                                 | -              | void (noreturn)                            |
| 5      | sys_sbrk      | Allocates memory, aligned to 4KB.                         | 0x05              | size_t len            | -                            | -                      | -          | -                                 | -              | void\* (pointer, NULL on error)            |
| 6      | sys_uname     | Retrieves system information.                             | 0x06              | struct utsname \*buf  | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 7      | sys_break     | Triggers a trap at the current instruction (noreturn).    | 0x07              | -                     | -                            | -                      | -          | -                                 | -              | void (noreturn)                            |
| 8      | sys_stat      | Gets file information.                                    | 0x08              | int fd                | struct stat \*buf            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 9      | sys_uptime    | Returns the system uptime in milliseconds.                | 0x09              | -                     | -                            | -                      | -          | -                                 | -              | uint64_t (uptime ms)                       |
| 10     | sys_exec      | Spawns a new process from an ELF file.                    | 0x0a              | const char \*args     | int pipefd[3]                | -                      | -          | -                                 | -              | pid_t (pid on success, -1 on error)        |
| 11     | sys_getcwd    | Gets the absolute path of the current working directory.  | 0x0b              | char \*buf            | size_t buf_len               | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 12     | sys_chdir     | Changes the current working directory.                    | 0x0c              | const char \*path     | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 13     | sys_free      | Frees memory allocated by sbrk.                           | 0x0d              | void \*ptr            | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 14     | sys_wait      | Waits for the process with the given pid to exit.         | 0x0e              | pid_t pid             | int \*status (`sys/wait.h`)  | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 15     | sys_sbrksz    | Gets the size of memory acquired by sbrk.                 | 0x0f              | const void \*target   | -                            | -                      | -          | -                                 | -              | size_t (size, 0 on error)                  |
| 16     | sys_getpid    | Returns the pid of the current process.                   | 0x10              | -                     | -                            | -                      | -          | -                                 | -              | pid_t (current pid)                        |
| 17     | sys_getenames | Lists entry names in a directory, NUL-separated.          | 0x11              | const char \*path     | char \*buf                   | size_t buf_len         | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 18     | sys_iomsg     | Sends a generic I/O message for advanced operations.      | 0x12              | const void \*msgbuf   | void \*replymsgbuf           | size_t replymsgbuf_len | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 19     | sys_socket    | Creates an endpoint for communication.                    | 0x13              | int domain            | int type                     | int protocol           | -          | -                                 | -              | int (sockfd, -1 on error)                  |
| 20     | sys_bind      | Binds a port to a socket.                                 | 0x14              | int sockfd            | const struct sockaddr \*addr | size_t addrlen         | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 21     | sys_sendto    | Sends a message on a socket.                              | 0x15              | int sockfd            | const void \*buf             | size_t len             | int flags  | const struct sockaddr \*dest_addr | size_t addrlen | int (sent bytes, -1 on error)              |
| 22     | sys_recvfrom  | Receives a datagram and writes its sender to src_addr.    | 0x16              | int sockfd            | void \*buf                   | size_t len             | int flags  | struct sockaddr \*src_addr        | size_t addrlen | int (received bytes, -1 on error)          |
| 23     | sys_send      | Sends a message on a connected socket.                    | 0x17              | int sockfd            | const void \*buf             | size_t len             | int flags  | -                                 | -              | int (sent bytes, -1 on error)              |
| 24     | sys_recv      | Receives a message from a connected socket.               | 0x18              | int sockfd            | void \*buf                   | size_t len             | int flags  | -                                 | -              | int (received bytes, -1 on error)          |
| 25     | sys_connect   | Initiates a connection on a socket.                       | 0x19              | int sockfd            | const struct sockaddr \*addr | size_t addrlen         | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 26     | sys_listen    | Listens for connections on a socket.                      | 0x1a              | int sockfd            | int backlog                  | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 27     | sys_accept    | Accepts a connection on a socket.                         | 0x1b              | int sockfd            | struct sockaddr \*addr       | size_t \*addrlen       | -          | -                                 | -              | int (sockfd, -1 on error)                  |
| 28     | sys_pipe      | Creates an unnamed pipe.                                  | 0x1c              | int pipefd[2]         | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 29     | sys_lseek     | Repositions a file descriptor's offset.                   | 0x1d              | int fd                | off_t offset                 | int whence             | -          | -                                 | -              | off_t (new offset, -1 on error)            |
| 30     | sys_fork      | Creates a new process by duplicating the calling process. | 0x1e              | -                     | -                            | -                      | -          | -                                 | -              | pid_t (child pid, 0 in child, -1 on error) |
| 31     | sys_sleep     | Suspends the calling process for at least `ms` ms.        | 0x1f              | uint64_t ms           | -                            | -                      | -          | -                                 | -              | void                                       |
| 32     | sys_yield     | Yields the CPU to another ready process.                  | 0x20              | -                     | -                            | -                      | -          | -                                 | -              | void                                       |
| 33     | sys_mkfifo    | Creates a named pipe.                                     | 0x21              | const char \*path     | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 34     | sys_unlink    | Removes a named pipe from the file system.                | 0x22              | const char \*path     | -                            | -                      | -          | -                                 | -              | int (0 on success, -1 on error)            |
| 35     | sys_poll      | Waits until any of the descriptors becomes readable.      | 0x23              | pollfd \*fds          | size_t nfds                  | int64_t timeout_ms     | -          | -                                 | -              | int (ready fds, 0 on timeout, -1 on error) |

## Notes

### sys_open flags (`syscalls.h`)

| flag                 | value | description                                                         |
| -------------------- | ----- | ------------------------------------------------------------------- |
| `OPEN_FLAG_NONE`     | 0x0   | -                                                                   |
| `OPEN_FLAG_CREATE`   | 0x1   | Creates the file if it does not exist.                              |
| `OPEN_FLAG_READ`     | 0x2   | Opens the read end of a named pipe.                                 |
| `OPEN_FLAG_WRITE`    | 0x4   | Opens the write end of a named pipe.                                |
| `OPEN_FLAG_NONBLOCK` | 0x8   | `sys_read` returns 0 immediately instead of blocking when no data. |

### sys_read

- stdin (TTY) is line-buffered: it blocks until a line is entered and returns at most one line, including the trailing `\n`. Line editing (backspace, `^C`) is handled by the kernel.
- Pipes and input devices block until data is available. A return value of 0 means EOF (or no data with `OPEN_FLAG_NONBLOCK`).

### sys_exec

- `pipefd[3]` replaces stdin / stdout / stderr of the child. `-1` keeps the TTY. Use `EXEC_PIPE_NONE` for no redirection.

### sys_wait (`sys/wait.h`)

`status` receives a POSIX-style wait status. Pass `NULL` to discard it.

| macro                 | description                                         |
| --------------------- | --------------------------------------------------- |
| `WIFEXITED(status)`   | True if the process exited via `sys_exit`.          |
| `WEXITSTATUS(status)` | Exit code (0-255). `exit(-1)` is reported as 255.   |
| `WIFSIGNALED(status)` | True if the process was killed by the kernel.       |
| `WTERMSIG(status)`    | Signal number (`SIGINT` for `^C`, `SIGSEGV` for faults). |

- `system()` returns the wait status, `W_EXITCODE(127, 0)` if the command could not be executed, and -1 only if waiting failed.
- A Rust app that panics exits with code 101.

### Sockets

- `struct sockaddr_in` uses **host byte order** for `sin_port` and `sin_addr.s_addr` (unlike POSIX).
- UDP `sys_recvfrom` is non-blocking: it returns 0 when no datagram is queued. Each call returns one datagram; bytes beyond `len` are discarded. `src_addr` must be non-NULL and `addrlen >= sizeof(struct sockaddr_in)`.
- Ephemeral ports are assigned round-robin from 49152-65535.

### sys_poll (`poll.h`)

- Only `POLLIN` is supported.
- `timeout_ms`: 0 returns immediately, negative waits forever, positive waits at least that long.

### Timing

- `sys_sleep` and `sys_poll` timeouts are rounded up to the 10ms timer tick, plus one tick.

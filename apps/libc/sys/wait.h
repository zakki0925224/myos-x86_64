#ifndef _SYS_WAIT_H
#define _SYS_WAIT_H

#define W_EXITCODE(ret, sig) (((ret) & 0xff) << 8 | ((sig) & 0x7f))

#define WEXITSTATUS(status) (((status) >> 8) & 0xff)
#define WTERMSIG(status) ((status) & 0x7f)
#define WIFEXITED(status) (WTERMSIG(status) == 0)
#define WIFSIGNALED(status) (WTERMSIG(status) != 0)

#endif

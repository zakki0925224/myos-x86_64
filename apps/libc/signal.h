#ifndef _SIGNAL_H
#define _SIGNAL_H

#define SIGINT 2
#define SIGILL 4
#define SIGFPE 8
#define SIGSEGV 11
#define SIGTERM 15
#define SIG_DFL ((void (*)(int))0)
#define SIG_ERR ((void (*)(int)) - 1)
#define SIG_IGN ((void (*)(int))1)

typedef void (*sighandler_t)(int);

sighandler_t signal(int signum, sighandler_t handler);

#endif

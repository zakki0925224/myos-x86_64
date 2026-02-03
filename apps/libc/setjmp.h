#ifndef _SETJMP_H
#define _SETJMP_H

#include <stdint.h>

typedef struct {
    uint64_t rbx;
    uint64_t rbp;
    uint64_t r12;
    uint64_t r13;
    uint64_t r14;
    uint64_t r15;
    uint64_t rsp;
    uint64_t rip;
} jmp_buf[1];

extern int setjmp(jmp_buf env);
extern void longjmp(jmp_buf env, int val);

#endif

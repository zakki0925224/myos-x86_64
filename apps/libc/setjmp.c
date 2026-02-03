#include "setjmp.h"

/*
 * x86_64 System V ABI setjmp/longjmp implementation
 * jmp_buf is assumed to be an array of uint64_t based on setjmp.h
 *
 * Saved registers: rbx, rbp, r12, r13, r14, r15, rsp, rip
 */

__attribute__((naked)) int setjmp(jmp_buf env) {
    __asm__ volatile(
        "movq %rbx, (%rdi)\n"
        "movq %rbp, 8(%rdi)\n"
        "movq %r12, 16(%rdi)\n"
        "movq %r13, 24(%rdi)\n"
        "movq %r14, 32(%rdi)\n"
        "movq %r15, 40(%rdi)\n"
        "leaq 8(%rsp), %rdx\n"  // caller's RSP (skipping return address)
        "movq %rdx, 48(%rdi)\n"
        "movq (%rsp), %rdx\n"  // caller's RIP (return address)
        "movq %rdx, 56(%rdi)\n"
        "xorl %eax, %eax\n"  // return 0
        "retq\n");
}

__attribute__((naked)) void longjmp(jmp_buf env, int val) {
    __asm__ volatile(
        "movq %rsi, %rax\n"  // return val
        "testl %eax, %eax\n"
        "jnz 1f\n"
        "incl %eax\n"  // if val==0, return 1
        "1:\n"
        "movq (%rdi), %rbx\n"
        "movq 8(%rdi), %rbp\n"
        "movq 16(%rdi), %r12\n"
        "movq 24(%rdi), %r13\n"
        "movq 32(%rdi), %r14\n"
        "movq 40(%rdi), %r15\n"
        "movq 48(%rdi), %rsp\n"
        "jmpq *56(%rdi)\n"  // jump to saved RIP
    );
}

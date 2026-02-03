#ifndef _MATH_H
#define _MATH_H

extern long pow(long base, long exp);
extern long ldexp(long x, int exp);
extern long floor(long x);
extern long frexp(long x, int* exp);
extern long fmod(long x, long y);

#define HUGE_VAL 0

#endif

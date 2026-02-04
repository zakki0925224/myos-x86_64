#ifndef _MATH_H
#define _MATH_H

long pow(long base, long exp);
long ldexp(long x, int exp);
long floor(long x);
long frexp(long x, int* exp);
long fmod(long x, long y);

#define HUGE_VAL 0

#endif

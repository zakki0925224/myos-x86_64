#include "math.h"

#include "stdlib.h"

// x * 2^exp
long ldexp(long x, int exp) {
    if (exp == 0) return x;
    if (exp > 0) return x << exp;
    return x >> (-exp);
}

// x^y
long pow(long base, long exp) {
    long res = 1;
    while (exp > 0) {
        if (exp % 2 == 1) res *= base;
        base *= base;
        exp /= 2;
    }
    return res;
}

long floor(long x) {
    return x;  // Integer identity
}

long frexp(long x, int* exp) {
    *exp = 0;
    return x;
}

long fmod(long x, long y) {
    if (y == 0) return 0;
    return x % y;
}

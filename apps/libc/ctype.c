#include "ctype.h"

int isdigit(int c) {
    return (c >= '0' && c <= '9');
}

int isalpha(int c) {
    return (islower(c) || isupper(c));
}

int isspace(int c) {
    return ((c == ' ') || (c == '\n') || (c == '\t'));
}

int isupper(int c) {
    return (c >= 'A' && c <= 'Z');
}

int islower(int c) {
    return (c >= 'a' && c <= 'z');
}

int isxdigit(int c) {
    return (isdigit(c) || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'));
}

int isalnum(int c) {
    return (isalpha(c) || isdigit(c));
}

int toupper(int c) {
    if (islower(c)) {
        return (c - 'a' + 'A');
    }

    return c;
}

int tolower(int c) {
    if (isupper(c)) {
        return (c - 'A' + 'a');
    }

    return c;
}

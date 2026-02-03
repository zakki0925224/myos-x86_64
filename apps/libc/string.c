#include "string.h"

#include "ctype.h"
#include "stdlib.h"

int strcmp(const char* s1, const char* s2) {
    while (*s1 != '\0' && *s2 != '\0' && *s1 == *s2) {
        s1++;
        s2++;
    }
    return *s1 - *s2;
}

size_t strlen(const char* str) {
    size_t res = 0;
    while (*str++) {
        res++;
    }
    return res;
}

int split(char* str, const char regex, char** buf, size_t buflen) {
    int s_len = strlen(str);
    int len = 1;
    int i;

    buf[0] = str;

    for (i = 1; i < s_len; i++) {
        if (str[i] == regex) {
            str[i] = '\0';
            buf[len] = &str[i + 1];
            len++;

            if ((int)buflen <= len) {
                break;
            }
        }
    }

    return len;
}

char* concatenate(const char* strs[], int len, const char* delimiter) {
    int total_len = 0;
    int delimiter_len = strlen(delimiter);
    int i, j, k = 0;

    for (i = 0; i < len; i++) {
        total_len += strlen(strs[i]);
        if (i < len - 1) {
            total_len += delimiter_len;
        }
    }

    char* str = (char*)malloc(total_len + 1);
    if (str == NULL) {
        return NULL;
    }

    for (i = 0; i < len; i++) {
        for (j = 0; j < strlen(strs[i]); j++) {
            str[k++] = strs[i][j];
        }

        if (i < len - 1) {
            for (j = 0; j < delimiter_len; j++) {
                str[k++] = delimiter[j];
            }
        }
    }

    str[k] = '\0';
    return str;
}

void replace(char* src, const char target, const char replace) {
    int i = 0;
    int str_len = strlen(src);

    while (i < str_len) {
        if (src[i] == target) {
            src[i] = replace;
        }

        i++;
    }
}

int is_ascii(const char c) {
    return c >= 0 && c <= 127;
}

int memcmp(const void* s1, const void* s2, size_t n) {
    const unsigned char* p1 = (const unsigned char*)s1;
    const unsigned char* p2 = (const unsigned char*)s2;

    while (n-- > 0) {
        if (*p1 != *p2)
            return *p1 - *p2;
        p1++;
        p2++;
    }

    return 0;
}

void* memcpy(void* dest, const void* src, size_t len) {
    char* d = (char*)dest;
    const char* s = (char*)src;

    while (len--)
        *d++ = *s++;

    return dest;
}

void* memset(void* dest, int val, size_t len) {
    unsigned char* ptr = (unsigned char*)dest;

    while (len-- > 0)
        *ptr++ = val;

    return dest;
}

void* memmove(void* dest, const void* src, size_t len) {
    char* d = (char*)dest;
    const char* s = (char*)src;

    if (d < s) {
        while (len--)
            *d++ = *s++;
    } else {
        char* lasts = (char*)(s + (len - 1));
        char* lastd = (char*)(d + (len - 1));

        while (len--)
            *lastd-- = *lasts--;
    }

    return dest;
}

int strcasecmp(const char* s1, const char* s2) {
    int d = 0;

    for (;;) {
        const int c1 = tolower(*s1++);
        const int c2 = tolower(*s2++);

        if (((d = c1 - c2) != 0) || (c2 == '\0'))
            break;
    }

    return d;
}

int strncasecmp(const char* s1, const char* s2, size_t n) {
    int d = 0;

    for (; n != 0; n--) {
        const int c1 = tolower(*s1++);
        const int c2 = tolower(*s2++);

        if (((d = c1 - c2) != 0) || (c2 == '\0'))
            break;
    }

    return d;
}

char* strchr(const char* s1, int i) {
    const unsigned char* s = (const unsigned char*)s1;
    unsigned char c = (unsigned char)i;

    while (*s && *s != c)
        s++;

    if (*s == c)
        return (char*)s;

    return NULL;
}

char* strrchr(const char* s, int i) {
    const char* last = NULL;

    if (i) {
        while ((s = strchr(s, i))) {
            last = s;
            s++;
        }
    } else {
        last = strchr(s, i);
    }

    return (char*)last;
}

int strncmp(const char* s1, const char* s2, size_t n) {
    if (n == 0)
        return 0;

    while (n-- > 0 && *s1 == *s2) {
        if (n == 0 || *s1 == '\0')
            return 0;

        s1++;
        s2++;
    }

    return (*(unsigned char*)s1 - *(unsigned char*)s2);
}

char* strcpy(char* dest, const char* src) {
    char* d = dest;
    while ((*d++ = *src++) != 0);
    return dest;
}

char* strncpy(char* dst, const char* src, size_t n) {
    if (n == 0)
        return dst;

    char* d = dst;
    const char* s = src;

    do {
        if ((*d++ = *s++) == 0) {
            while (--n != 0)
                *d++ = 0;

            break;
        }
    } while (--n != 0);

    return dst;
}

char* strdup(const char* s) {
    size_t len = strlen(s) + 1;
    char* mem = (char*)malloc(len);

    if (mem == NULL)
        return NULL;

    memcpy(mem, s, len);
    return mem;
}

char* strstr(const char* s1, const char* s2) {
    size_t i;
    int c = s2[0];

    if (c == 0)
        return (char*)s1;

    for (; s1[0] != '\0'; s1++) {
        if (s1[0] != c)
            continue;

        for (i = 1; s2[i] != 0; i++) {
            if (s1[i] != s2[i])
                break;
        }

        if (s2[i] == '\0')
            return (char*)s1;
    }

    return NULL;
}

size_t strspn(const char* s, const char* accept) {
    const char* p = s;
    const char* a;

    while (*p) {
        for (a = accept; *a; a++) {
            if (*p == *a)
                break;
        }
        if (*a == '\0')
            return p - s;
        p++;
    }

    return p - s;
}

char* strpbrk(const char* s, const char* accept) {
    while (*s) {
        const char* a = accept;
        while (*a) {
            if (*s == *a++)
                return (char*)s;
        }
        s++;
    }
    return NULL;
}

char* strerror(int errnum) {
    return "Unknown error";
}

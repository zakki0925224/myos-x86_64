#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

#include "string.h"
#include "syscalls.h"

#define BUF_SIZE 1000
static char BUF[BUF_SIZE];

int write_buf(char* buf, int buf_len, int write_len, char c) {
    if (write_len >= buf_len)
        return -1;

    buf[write_len++] = c;
    return write_len;
}

int _printf(char* buf, int buf_len, const char* fmt, va_list ap) {
    int ret = 0;
    int str_len = strlen(fmt);
    int str_i = 0;
    int buf_i = 0;

    while (buf_i != -1 && ret != -1) {
        if (str_i >= str_len || buf_i >= buf_len - 1) {
            ret = write_buf(buf, buf_len, buf_i, '\0');
            buf_i = -1;
            break;
        }

        char c = fmt[str_i++];

        if (c != '%') {
            buf_i = write_buf(buf, buf_len, buf_i, c);
            if (buf_i == -1) {
                ret = -1;
                break;
            }
            continue;
        }

        char nc = fmt[str_i++];

        bool zero_fill = false;
        int min_width = 0;
        int precision = -1;

        while ((nc >= '0' && nc <= '9') || nc == '.') {
            if (nc == '.') {
                precision = 0;
            } else if (precision >= 0) {
                precision = precision * 10 + (nc - '0');
            } else if (nc == '0' && min_width == 0) {
                zero_fill = true;
            } else {
                min_width = min_width * 10 + (nc - '0');
            }

            nc = fmt[str_i++];
        }

        int length = 0;  // 0: default, 1: l, 2: ll
        while (nc == 'l') {
            length++;
            nc = fmt[str_i++];
        }

        switch (nc) {
            case 'd':
            case 'i': {
                int64_t va_num;
                if (length == 2) {
                    va_num = va_arg(ap, long long);
                } else if (length == 1) {
                    va_num = va_arg(ap, long);
                } else {
                    va_num = va_arg(ap, int);
                }

                bool is_negative = va_num < 0;
                if (is_negative) {
                    buf_i = write_buf(buf, buf_len, buf_i, '-');
                    va_num = -va_num;
                }

                char num_str[32];
                int num_len = 0;

                if (va_num == 0) {
                    num_str[num_len++] = '0';
                } else {
                    while (va_num > 0 && num_len < sizeof(num_str)) {
                        num_str[num_len++] = '0' + (va_num % 10);
                        va_num /= 10;
                    }
                }

                for (int i = 0; i < (min_width > num_len ? min_width - num_len : 0); i++) {
                    char fill_char = zero_fill ? '0' : ' ';
                    buf_i = write_buf(buf, buf_len, buf_i, fill_char);
                }

                for (int i = 0; i < (precision > num_len ? precision - num_len : 0); i++) {
                    buf_i = write_buf(buf, buf_len, buf_i, '0');
                }

                for (int i = num_len - 1; i >= 0; i--) {
                    buf_i = write_buf(buf, buf_len, buf_i, num_str[i]);
                }

                break;
            }

            case 'u': {
                uint64_t va_num;
                if (length == 2) {
                    va_num = va_arg(ap, unsigned long long);
                } else if (length == 1) {
                    va_num = va_arg(ap, unsigned long);
                } else {
                    va_num = va_arg(ap, unsigned int);
                }

                char num_str[32];
                int num_len = 0;

                if (va_num == 0) {
                    num_str[num_len++] = '0';
                } else {
                    while (va_num > 0 && num_len < sizeof(num_str)) {
                        num_str[num_len++] = '0' + (va_num % 10);
                        va_num /= 10;
                    }
                }

                for (int i = 0; i < (min_width > num_len ? min_width - num_len : 0); i++) {
                    char fill_char = zero_fill ? '0' : ' ';
                    buf_i = write_buf(buf, buf_len, buf_i, fill_char);
                }

                for (int i = 0; i < (precision > num_len ? precision - num_len : 0); i++) {
                    buf_i = write_buf(buf, buf_len, buf_i, '0');
                }

                for (int i = num_len - 1; i >= 0; i--) {
                    buf_i = write_buf(buf, buf_len, buf_i, num_str[i]);
                }
                break;
            }

            case 'p':
            case 'x':
            case 'X': {
                uint64_t va_num;
                if (nc == 'p') {
                    va_num = (uintptr_t)va_arg(ap, void*);
                } else if (length == 2) {
                    va_num = va_arg(ap, unsigned long long);
                } else if (length == 1) {
                    va_num = va_arg(ap, unsigned long);
                } else {
                    va_num = va_arg(ap, unsigned int);
                }

                char num_str[32];
                int num_len = 0;

                if (va_num == 0) {
                    num_str[num_len++] = '0';
                } else {
                    while (va_num > 0 && num_len < sizeof(num_str)) {
                        int digit = va_num % 16;
                        if (digit < 10) {
                            num_str[num_len++] = '0' + digit;
                        } else {
                            num_str[num_len++] = (nc == 'x' || nc == 'p' ? 'a' : 'A') + digit - 10;
                        }
                        va_num /= 16;
                    }
                }

                for (int i = 0; i < (min_width > num_len ? min_width - num_len : 0); i++) {
                    char fill_char = zero_fill ? '0' : ' ';
                    buf_i = write_buf(buf, buf_len, buf_i, fill_char);
                }

                for (int i = 0; i < (precision > num_len ? precision - num_len : 0); i++) {
                    buf_i = write_buf(buf, buf_len, buf_i, '0');
                }

                for (int i = num_len - 1; i >= 0; i--) {
                    buf_i = write_buf(buf, buf_len, buf_i, num_str[i]);
                }

                break;
            }

            case 'c': {
                char va_c = va_arg(ap, int);
                buf_i = write_buf(buf, buf_len, buf_i, va_c);
                break;
            }

            case 's': {
                const char* va_s = va_arg(ap, char*);
                if (va_s == NULL) {
                    ret = -1;
                    break;
                }

                int va_s_len = strlen(va_s);
                for (int i = 0; i < va_s_len; i++) {
                    buf_i = write_buf(buf, buf_len, buf_i, va_s[i]);
                }
                break;
            }

            case '%':
                buf_i = write_buf(buf, buf_len, buf_i, '%');
                break;

            default:
                ret = -1;
                break;
        }
    }

    return ret;
}

int printf(const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int ret = _printf(BUF, BUF_SIZE, fmt, ap);

    if (ret == -1) {
        ret = _printf(BUF, BUF_SIZE, "<PRINTF ERROR>\n", ap);
    }
    va_end(ap);

    if (ret != -1) {
        ret = sys_write(FDN_STDOUT, BUF, strlen(BUF));
    }

    return ret;
}

int vsnprintf(char* buf, size_t bufsize, const char* format, va_list arg) {
    int ret = _printf(buf, bufsize, format, arg);

    if (ret != -1) {
        ret = strlen(buf);
    }

    return ret;
}

int snprintf(char* buf, size_t size, const char* format, ...) {
    va_list ap;
    va_start(ap, format);
    int ret = _printf(buf, size, format, ap);
    va_end(ap);

    if (ret != -1) {
        ret = strlen(buf);
    }

    return ret;
}

int fprintf(FILE* stream, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int ret = vfprintf(stream, fmt, ap);
    va_end(ap);
    return ret;
}

int vfprintf(FILE* stream, const char* fmt, va_list ap) {
    if (stream == NULL) {
        return -1;
    }

    char buf[1024];
    int len = vsnprintf(buf, sizeof(buf), fmt, ap);

    if (len < 0) {
        return -1;
    }

    size_t written = fwrite(buf, 1, len, stream);
    if (written < (size_t)len) {
        return -1;
    }

    return len;
}

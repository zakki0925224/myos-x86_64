#include "stdio.h"

#include <stddef.h>

#include "stat.h"
#include "stdlib.h"
#include "string.h"
#include "syscalls.h"

static f_stat __stdin_stat = {.size = 0};
static f_stat __stdout_stat = {.size = 0};
static f_stat __stderr_stat = {.size = 0};

static FILE __stdin = {.fd = FDN_STDIN, .stat = &__stdin_stat, .buf = NULL, .pos = 0, .flags = 0};
static FILE __stdout = {.fd = FDN_STDOUT, .stat = &__stdout_stat, .buf = NULL, .pos = 0, .flags = 0};
static FILE __stderr = {.fd = FDN_STDERR, .stat = &__stderr_stat, .buf = NULL, .pos = 0, .flags = 0};

FILE* stdin = &__stdin;
FILE* stdout = &__stdout;
FILE* stderr = &__stderr;

void exit(int status) {
    sys_exit((uint64_t)status);
}

FILE* fopen(const char* filepath, const char* mode) {
    uint32_t flags = OPEN_FLAG_NONE;

    if (strcmp(mode, "w") == 0) {
        flags |= OPEN_FLAG_CREATE;
    }

    int fd = sys_open(filepath, flags);
    if (fd == -1)
        return NULL;

    f_stat* stat = (f_stat*)malloc(sizeof(f_stat));
    if (sys_stat(fd, stat) == -1) {
        free(stat);
        sys_close(fd);
        return NULL;
    }

    FILE* file = (FILE*)malloc(sizeof(FILE));
    file->fd = fd;
    file->buf = NULL;
    file->stat = stat;
    file->pos = 0;
    file->flags = 0;
    return file;
}

int fclose(FILE* stream) {
    if (stream == NULL)
        return -1;

    int64_t res = sys_close(stream->fd);
    if (res == -1)
        return -1;

    if (stream->buf != NULL)
        free(stream->buf);

    if (stream->stat != NULL)
        free(stream->stat);

    free(stream);
    return 0;
}

long int ftell(FILE* stream) {
    if (stream == NULL)
        return -1;

    return stream->pos;
}

int fflush(FILE* stream) {
    if (stream == NULL)
        return -1;

    if (stream->buf == NULL || stream->pos == 0)
        return 0;

    int ret = sys_write(stream->fd, stream->buf, stream->pos);
    if (ret == -1)
        return -1;
    free(stream->buf);
    stream->buf = NULL;
    stream->pos = 0;
    return 0;
}

int puts(const char* c) {
    int ret = sys_write(FDN_STDOUT, c, strlen(c));

    if (ret == -1)
        return -1;

    ret = sys_write(FDN_STDOUT, "\n", 1);
    if (ret == -1)
        return -1;

    return 0;
}

int putchar(int c) {
    return printf("%c", c);
}

char getchar(void) {
    char c;
    int ret = sys_read(FDN_STDIN, &c, 1);
    if (ret == -1)
        return EOF;
    return c;
}

int sscanf(const char* buf, const char* fmt, ...) {
    printf("[DEBUG]sscanf called\n");
    return -1;
}

size_t fread(void* buf, size_t size, size_t count, FILE* stream) {
    if (size == 0 || count == 0)
        return 0;

    if (stream == NULL)
        return 0;

    if (stream->fd == FDN_STDIN) {
        int res = sys_read(stream->fd, buf, size * count);
        if (res == -1) {
            stream->flags |= _FILE_ERR_FLAG;
            return 0;
        }
        if (res == 0) {
            stream->flags |= _FILE_EOF_FLAG;
        }
        return res / size;
    }

    size_t f_size = stream->stat->size;

    if (stream->buf == NULL) {
        stream->buf = (char*)malloc(f_size);
        if (stream->buf == NULL)
            return 0;

        if (sys_read(stream->fd, stream->buf, f_size) == -1) {
            free(stream->buf);
            return 0;
        }
    }

    size_t remaining = f_size - stream->pos;
    size_t bytes_to_read = size * count > remaining ? remaining : size * count;

    memcpy(buf, stream->buf + stream->pos, bytes_to_read);
    stream->pos += bytes_to_read;

    if (bytes_to_read < size * count) {
        stream->flags |= _FILE_EOF_FLAG;
    }

    return bytes_to_read / size;
}

int fseek(FILE* stream, long int offset, int whence) {
    if (stream == NULL)
        return -1;

    // fseek clears EOF flag
    stream->flags &= ~_FILE_EOF_FLAG;

    size_t f_size = stream->stat->size;
    switch (whence) {
        case SEEK_SET:
            if (offset < 0 || offset > f_size)
                return -1;
            stream->pos = offset;
            break;
        case SEEK_CUR:
            if (stream->pos + offset < 0 || stream->pos + offset > f_size)
                return -1;
            stream->pos += offset;
            break;
        case SEEK_END:
            if (f_size + offset < 0)
                return -1;
            stream->pos = f_size + offset;
            break;
        default:
            return -1;
    }

    return 0;
}

size_t fwrite(const void* buf, size_t size, size_t count, FILE* stream) {
    if (size == 0 || count == 0)
        return 0;

    if (stream == NULL)
        return 0;

    if (stream->fd == FDN_STDOUT || stream->fd == FDN_STDERR) {
        int res = sys_write(stream->fd, buf, size * count);
        if (res == -1)
            return 0;
        return count;
    }

    size_t bytes_to_write = size * count;
    if (stream->buf == NULL) {
        stream->buf = (char*)malloc(bytes_to_write);
        if (stream->buf == NULL)
            return 0;
    } else {
        stream->buf = (char*)realloc(stream->buf, stream->pos + bytes_to_write);
        if (stream->buf == NULL)
            return 0;
    }

    memcpy(stream->buf + stream->pos, buf, bytes_to_write);
    stream->pos += bytes_to_write;

    return count;
}

int setvbuf(FILE* stream, char* buf, int mode, size_t size) {
    printf("[DEBUG]setvbuf called\n");
    return 0;
}

void clearerr(FILE* stream) {
    if (stream != NULL) {
        stream->flags = 0;
    }
}

int ferror(FILE* stream) {
    if (stream == NULL) {
        return 0;
    }
    return (stream->flags & _FILE_ERR_FLAG) != 0;
}

int feof(FILE* stream) {
    if (stream == NULL) {
        return 0;
    }
    return (stream->flags & _FILE_EOF_FLAG) != 0;
}

FILE* tmpfile(void) {
    printf("[DEBUG]tmpfile called\n");
    return NULL;
}

int ungetc(int c, FILE* stream) {
    printf("[DEBUG]ungetc called\n");
    return EOF;
}

int getc(FILE* stream) {
    unsigned char c;
    if (fread(&c, 1, 1, stream) == 1) {
        return c;
    }
    return EOF;
}

char* fgets(char* s, int size, FILE* stream) {
    if (size <= 0) return NULL;

    char* p = s;
    int c;
    int i = 0;

    while (i < size - 1) {
        c = getc(stream);
        if (c == EOF) {
            break;
        }
        *p++ = (char)c;
        i++;
        if (c == '\n') {
            break;
        }
    }

    if (i == 0) return NULL;

    *p = '\0';
    return s;
}

FILE* freopen(const char* filename, const char* mode, FILE* stream) {
    if (stream) {
        fclose(stream);
    }
    return fopen(filename, mode);
}

int fputs(const char* s, FILE* stream) {
    return fwrite(s, 1, strlen(s), stream);
}

#include "stdio.h"

#include <stddef.h>

#include "stat.h"
#include "stdlib.h"
#include "string.h"
#include "syscalls.h"

void exit(int status) {
    sys_exit((uint64_t)status);
}

int fprintf(FILE *stream, const char *fmt, ...) {
    printf("[DEBUG]fprintf called\n");
    return -1;
}

FILE *fopen(const char *filename, const char *mode) {
    // printf("[DEBUG]fopen called\n");
    uint32_t flags = OPEN_FLAG_NONE;

    if (strcmp(mode, "w") == 0) {
        flags |= OPEN_FLAG_CREATE;
    }

    int fd = sys_open(filename, flags);
    if (fd == -1)
        return NULL;

    f_stat *stat = (f_stat *)malloc(sizeof(f_stat));
    if (sys_stat(fd, stat) == -1) {
        free(stat);
        sys_close(fd);
        return NULL;
    }

    FILE *file = (FILE *)malloc(sizeof(FILE));
    file->fd = fd;
    file->buf = NULL;
    file->stat = stat;
    file->pos = 0;
    return file;
}

int fclose(FILE *stream) {
    // printf("[DEBUG]fclose called\n");
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

long int ftell(FILE *stream) {
    // printf("[DEBUG]ftell called\n");
    if (stream == NULL)
        return -1;

    return stream->pos;
}

int fflush(FILE *stream) {
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

int puts(const char *c) {
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

int vfprintf(FILE *stream, const char *fmt, va_list ap) {
    printf("[DEBUG]vfprintf called\n");
    return -1;
}

int sscanf(const char *buf, const char *fmt, ...) {
    printf("[DEBUG]sscanf called\n");
    return -1;
}

size_t fread(void *buf, size_t size, size_t count, FILE *stream) {
    // printf("[DEBUG]fread called\n");
    if (size == 0 || count == 0)
        return 0;

    if (stream == NULL)
        return 0;

    size_t f_size = stream->stat->size;

    if (stream->buf == NULL) {
        stream->buf = (char *)malloc(f_size);
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

    return bytes_to_read / size;
}

int fseek(FILE *stream, long int offset, int whence) {
    // printf("[DEBUG]fseek called\n");
    if (stream == NULL)
        return -1;

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

size_t fwrite(const void *buf, size_t size, size_t count, FILE *stream) {
    // printf("[DEBUG]fwrite called\n");
    if (size == 0 || count == 0)
        return 0;

    if (stream == NULL)
        return 0;

    size_t bytes_to_write = size * count;
    if (stream->buf == NULL) {
        stream->buf = (char *)malloc(bytes_to_write);
        if (stream->buf == NULL)
            return 0;
    } else {
        stream->buf = (char *)realloc(stream->buf, stream->pos + bytes_to_write);
        if (stream->buf == NULL)
            return 0;
    }

    memcpy(stream->buf + stream->pos, buf, bytes_to_write);
    stream->pos += bytes_to_write;

    return count;
}

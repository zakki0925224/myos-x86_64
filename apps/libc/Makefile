SRC_FILES := stdio.c stdlib.c string.c syscalls.c printf.c window.c main.c ctype.c sys/stat.c
OBJ_FILES := $(SRC_FILES:.c=.o)
LIB_FILE := libc.a

CC := gcc
CFLAGS := -Werror -g -m64 -nostdlib -fno-builtin -fno-stack-protector -std=c11

$(LIB_FILE): $(OBJ_FILES)
	ar rcs $@ $^
	nm $@

%.o: %.c
	$(CC) -c $< -o $@ $(CFLAGS)

clean:
	rm -f $(OBJ_FILES) $(LIB_FILE)

all: $(LIB_FILE)

.PHONY: clean all

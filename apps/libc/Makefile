SRC_FILES := stdio.c stdlib.c string.c syscalls.c printf.c window.c ctype.c sys/stat.c locale.c math.c setjmp.c time.c signal.c errno.c
OBJ_FILES := $(SRC_FILES:.c=.o)
LIB_FILE := libc.a

MAIN_OBJ := main.o
LIB_WITH_MAIN := libc_with_main.a

CC := gcc
CFLAGS := -Werror -g -m64 -nostdlib -fno-builtin -fno-stack-protector -std=c11

$(LIB_FILE): $(OBJ_FILES)
	ar rcs $@ $^
	nm $@

$(LIB_WITH_MAIN): $(OBJ_FILES) $(MAIN_OBJ)
	ar rcs $@ $^
	nm $@

app: $(LIB_WITH_MAIN)

%.o: %.c
	$(CC) -c $< -o $@ $(CFLAGS)

clean:
	rm -f $(OBJ_FILES) $(MAIN_OBJ) $(LIB_FILE) $(LIB_WITH_MAIN)

all: $(LIB_FILE)

.PHONY: clean all app

SRC_FILES := main.c
OBJ_FILES := $(SRC_FILES:.c=.o)
OUT_FILE := ../bin/lush

CC := gcc
CFLAGS := -I ../libc -I ../../third-party/lua-for-myos \
	-Werror -g -m64 -nostdlib -fno-builtin -fno-stack-protector \
	-mcmodel=large -mno-mmx -mno-sse -msoft-float -std=c11 \
	-DARCH_MYOS

LD := ld.lld
LDFLAGS := -z norelro --static --image-base=0xffff800000000000

LIBC_DIR := ../libc
LIBC := $(LIBC_DIR)/libc_with_main.a

LUA_DIR := ../../third-party/lua-for-myos
LIBLUA := $(LUA_DIR)/liblua.a

$(OUT_FILE): $(OBJ_FILES) $(LIBLUA) $(LIBC)
	mkdir -p ../bin
	$(LD) $(OBJ_FILES) $(LIBLUA) $(LIBC) -o $@ $(LDFLAGS)

$(LIBC):
	$(MAKE) -C $(LIBC_DIR) app

$(LIBLUA):
	$(MAKE) -C $(LUA_DIR) -f Makefile.myos liblua

%.o: %.c
	$(CC) -c $< -o $@ $(CFLAGS)

clean:
	rm -f $(OUT_FILE) $(OBJ_FILES)

all: $(OUT_FILE)

.PHONY: clean all

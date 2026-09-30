CC := gcc
CFLAGS := -I ../libc -Werror -g -m64 -nostdlib -fno-builtin -fno-stack-protector -mcmodel=large -mno-mmx -mno-sse -msoft-float -std=c11

LD := ld.lld
LDFLAGS := -z norelro --static --image-base=0xffff800000000000

LIBC_DIR := ../libc
LIBC := $(LIBC_DIR)/libc_with_main.a

BIN_DIR := ../bin

SRC_FILES := $(wildcard *.c)
OBJ_FILES := $(SRC_FILES:.c=.o)
OUT_FILES := $(addprefix $(BIN_DIR)/,$(SRC_FILES:.c=))

all: $(OUT_FILES)

$(BIN_DIR)/%: %.o $(LIBC)
	mkdir -p $(BIN_DIR)
	$(LD) $< $(LIBC) -o $@ $(LDFLAGS)

$(LIBC):
	$(MAKE) -C $(LIBC_DIR) app

%.o: %.c
	$(CC) -c $< -o $@ $(CFLAGS)

clean:
	rm -f $(OUT_FILES) $(OBJ_FILES)

.PHONY: all clean
.SECONDARY: $(OBJ_FILES)

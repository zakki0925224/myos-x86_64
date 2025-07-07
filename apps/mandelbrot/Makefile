FILE_NAME := mandelbrot
OUT_FILE := ../bin/$(FILE_NAME)
TARGET := x86_64-app
LIBC_DIR := ../libc

$(OUT_FILE):
	make -C $(LIBC_DIR) app
	cargo build --release
	cp target/$(TARGET)/release/$(FILE_NAME) $(OUT_FILE)

clean:
	rm -f $(OUT_FILE)

all: $(OUT_FILE)

.PHONY: clean all

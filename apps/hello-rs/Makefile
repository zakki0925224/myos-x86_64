FILE_NAME := hello-rs
OUT_FILE := ../bin/$(FILE_NAME)
TARGET := x86_64-app

$(OUT_FILE):
	cargo build --release
	cp target/$(TARGET)/release/$(FILE_NAME) $(OUT_FILE)

clean:
	rm -f $(OUT_FILE)

all: $(OUT_FILE)

.PHONY: clean all

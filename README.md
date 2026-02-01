# myos-x86_64

[![MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
![Language: Rust](https://img.shields.io/badge/Language-Rust-orange?logo=rust)
[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/zakki0925224/myos-x86_64)

**myos-x86_64** is a hobby operating system written in Rust.

This is a replacement project for the previous **[myos](https://github.com/zakki0925224/myos)**.

## Features

- [x] Written in Rust
- [x] My own UEFI boot loader using [uefi-rs](https://github.com/rust-osdev/uefi-rs)
- [x] x86_64 kernel
- [x] Paging
- [x] Multitasking
    - [x] Simple priority-based executor (for Rust's async runtime)
    - [x] Stack based single task scheduler (for userland tasks)
    - [ ] Multi task scheduler (for userland tasks) (WIP)
- Bus support
    - [x] PCI
    - [x] USB (xHCI)
- Device support
    - [x] PS/2 Keyboard and Mouse
    - [x] UART 16550A
    - [x] RTL8139
    - [x] PC Speaker
    - [x] USB HID Keyboard
    - [x] USB HID Tablet
- Timer support
    - [x] Local APIC timer (main timer)
    - [x] ACPI PM timer
    - [x] TSC
- File system
    - [x] VFS (Virtual File System)
    - [x] FAT32 (Read only)
    - [x] Device file system (/dev)
    - [x] Initramfs (FAT32, read/write in memory)
- Networking
    - [x] Ethernet (Raw frames)
    - [x] ARP
    - [x] IPv4
    - [x] ICMP
    - [x] UDP
    - [x] TCP
    - [x] Socket API
- Simple Window Manager
- [Userland applications](/apps/) implemented in C or Rust (libc for myos available [here](/apps/libc/))
    - [x] [Shell](/apps/sh/)
    - [x] [Web browser](/apps/web)
    - [x] DOOM challenge!
    - (and others...)

## Screenshots

![](https://github.com/user-attachments/assets/7cc7d545-b3ca-4042-b145-73a909834c13)
![](https://github.com/zakki0925224/myos-x86_64/assets/49384910/b134ef0a-c94e-46f8-a578-a6e160747fae)
![](https://github.com/zakki0925224/myos-x86_64/assets/49384910/fce1c2e4-f56b-46fa-8530-9eeec6069591)

## Third party

- OVMF from [EDK II](https://github.com/tianocore/edk2.git) (included)
- [Cozette](https://github.com/slavfox/Cozette.git) (download released binary when build)
- [QEMU](https://gitlab.com/qemu-project/qemu.git) (for debugging)
- [doom-for-myos](https://github.com/zakki0925224/doom-for-myos) (forked from [ozkl/doomgeneric](https://github.com/ozkl/doomgeneric))
- [doom1.wad](https://distro.ibiblio.org/slitaz/sources/packages/d/doom1.wad)

## How to run

### Minimum packages required to build and run

- For build kernel
    - git
    - rustup (and Rust toolchain)
    - python3
    - build-essential
    - lld
    - gcc-multilib
    - clang
    - qemu-system
    - qemu-utils
    - dosfstools
    - wget

- For build Cozette
    - python3-venv
    - bdf2psf (convert bdf file due to [bug in cozette.psf](https://github.com/slavfox/Cozette/issues/112))

- For build QEMU
    - ninja-build
    - meson
    - libglib2.0-dev
    - libsdl2-dev
    - libslirp-dev

```bash
# install required packages
$ sudo apt update && sudo apt install git python3 build-essential lld gcc-multilib clang qemu-system qemu-utils dosfstools wget python3-venv bdf2psf ninja-build meson libglib2.0-dev libsdl2-dev libslirp-dev

$ git clone https://github.com/zakki0925224/myos-x86_64.git
$ cd myos-x86_64
$ python3 ./task.py run
```

## How to run kernel test

```bash
$ cd myos-x86_64/kernel
$ cargo test
```

If you run `task.py` without an argument, you can see the list of commands.

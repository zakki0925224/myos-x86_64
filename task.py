import os
import subprocess
import sys

APPS_DIR = "apps"
APPS_LIBC_DIR = "libc"
OUTPUT_DIR = "build"
BOOTLOADER_DIR = "bootloader"
KERNEL_DIR = "kernel"
DUMP_DIR = "dump"
THIRD_PARTY_DIR = "third-party"
QEMU_DIR = "qemu"
DOOM_DIR = "doom-for-myos"
INITRAMFS_DIR = "initramfs"
MNT_DIR_PATH = "/mnt"

BOOTLOADER_FILE = "bootx64.efi"
KERNEL_FILE = "kernel.elf"
IMG_FILE = "myos.img"
ISO_FILE = "myos.iso"
FONT_FILE = "font.psf"
COZETTE_BDF = "cozette.bdf"
OVMF_CODE_FILE = "OVMF_CODE.fd"
QEMU_TRACE_FILE = "qemu_trace"
DOOM_WAD_FILE = "doom1.wad"
INITRAMFS_IMG_FILE = "initramfs.img"

GIT_CHECKOUT_TO_LATEST_TAG = "git fetch --tags && latestTag=$(git describe --tags `git rev-list --tags --max-count=1`) && git checkout $latestTag && git gc"

NETDEV_TAP = "tap0"
NETDEV_BR = "br0"
NETDEV_IP = "192.168.100.1/24"

QEMU_ARCH = "qemu-system-x86_64"
QEMU_TARGET_ARCH = "x86_64-softmmu"
QEMU_MONITOR_PORT = 5678
QEMU_GDB_PORT = 3333
QEMU_DEVICES = [
    # "-device nec-usb-xhci,id=xhci",
    # "-device usb-kbd",
    # "-device virtio-net,netdev=net0,mac=52:54:00:12:34:56 -netdev user,id=net0",
    "-device ahci,id=ahci",
    "-device ide-cd,drive=disk,bus=ahci.0,bootindex=1",
    "-device isa-debug-exit,iobase=0xf4,iosize=0x04",
    "-audiodev pa,id=speaker -machine pcspk-audiodev=speaker",
    f"-device rtl8139,netdev=net0, -netdev tap,id=net0,ifname={NETDEV_TAP},script=no,downscript=no -object filter-dump,id=f0,netdev=net0,file={DUMP_DIR}/dump.pcap",  # <- ping -4 192.168.100.2 / nc [-u] -zv 192.168.100.2 12345
]
QEMU_DRIVES = [
    f"-drive id=disk,if=none,format=raw,file=./{OUTPUT_DIR}/{IMG_FILE}",
    f"-drive if=pflash,format=raw,readonly=on,file=./{THIRD_PARTY_DIR}/{OVMF_CODE_FILE}",
]
QEMU_ARGS = [
    "-accel kvm",
    "-cpu host",
    "-no-reboot",
    "-no-shutdown",
    "-m 512M",
    "-serial mon:stdio",
    f"-monitor telnet::{QEMU_MONITOR_PORT},server,nowait",
    f"-gdb tcp::{QEMU_GDB_PORT}",
]

is_kernel_test = False
test_kernel_path = ""


def _qemu_cmd() -> str:
    global is_kernel_test

    qemu_args = " ".join(QEMU_ARGS)
    qemu_drives = " ".join(QEMU_DRIVES)
    qemu_devices = " ".join(QEMU_DEVICES)

    if is_kernel_test:
        qemu_args += " -display none"

    return f"{QEMU_ARCH} {qemu_args} {qemu_drives} {qemu_devices}"


def _own_qemu_cmd() -> str:
    return f"./{THIRD_PARTY_DIR}/{QEMU_DIR}/build/{_qemu_cmd()} --display sdl --trace events=./{QEMU_TRACE_FILE}"


def _git_submodule_update_cmd(path: str) -> str:
    return f"git submodule update --init --recursive {path}"


def _run_cmd(
    cmd: str,
    dir: str = "./",
    ignore_error: bool = False,
    check_qemu_exit_code: bool = False,
):
    print(f"\033[32m{cmd}\033[0m")
    cp = subprocess.run(cmd, shell=True, cwd=dir)
    exit_code = cp.returncode
    if check_qemu_exit_code:
        if exit_code == 33:  # EXIT_SUCCESS
            print("Received QEMU exit code: EXIT_SUCCESS")
            exit(0)
        elif exit_code == 35:  # EXIT_FAILURE
            print("Received QEMU exit code: EXIT_FAILURE")
            exit(1)
        else:
            print(f"Received QEMU exit code: Unknown({exit_code})")
            exit(1)

    if exit_code != 0 and not ignore_error:
        exit(exit_code)


def _init():
    _run_cmd(f"mkdir -p ./{OUTPUT_DIR}")
    _run_cmd(f"mkdir -p ./{DUMP_DIR}")
    _run_cmd(f"mkdir -p ./{APPS_DIR}/bin")


def _build_cozette():
    d = f"./{THIRD_PARTY_DIR}"

    if not os.path.exists(f"{d}/{FONT_FILE}"):
        _run_cmd(
            f'wget -qO- https://api.github.com/repos/slavfox/Cozette/releases/latest | grep "{COZETTE_BDF}" | cut -d : -f 2,3 | tr -d \\" | wget -O ./{COZETTE_BDF} -i -',
            dir=d,
            ignore_error=True,
        )
        _run_cmd(
            f"bdf2psf --fb ./{COZETTE_BDF} /usr/share/bdf2psf/standard.equivalents /usr/share/bdf2psf/fontsets/Uni2.512 512 ./{FONT_FILE}",
            dir=d,
        )
        _run_cmd(f"rm ./{COZETTE_BDF}", dir=d)


def _build_qemu():
    global is_kernel_test

    d = f"./{THIRD_PARTY_DIR}/{QEMU_DIR}"
    _run_cmd(_git_submodule_update_cmd(d))

    if is_kernel_test:
        return

    if not os.path.exists(f"{d}/build/{QEMU_ARCH}"):
        # run_cmd(f"{GIT_CHECKOUT_TO_LATEST_TAG}", dir=d)
        # extra_cflags = '--extra-cflags="-DDEBUG_RTL8139"'
        extra_cflags = ""
        _run_cmd(
            f"mkdir -p build && cd build && ../configure --target-list={QEMU_TARGET_ARCH} --enable-trace-backends=log --enable-sdl {extra_cflags} && make -j$(nproc)",
            dir=d,
        )


def _build_doom():
    # download doom1.wad
    if not os.path.exists(f"./{THIRD_PARTY_DIR}/{DOOM_WAD_FILE}"):
        _run_cmd(
            f"wget -P ./{THIRD_PARTY_DIR} https://distro.ibiblio.org/slitaz/sources/packages/d/doom1.wad"
        )

    d = f"./{THIRD_PARTY_DIR}/{DOOM_DIR}"
    _run_cmd(_git_submodule_update_cmd(d))
    _run_cmd("git checkout master", dir=d)
    _run_cmd("make -f Makefile.myos", dir=d)
    _run_cmd(f"cp {d}/doomgeneric ./{APPS_DIR}/bin/doom")
    _run_cmd(f"cp ./{THIRD_PARTY_DIR}/{DOOM_WAD_FILE} ./{INITRAMFS_DIR}")


def _build_bootloader():
    _run_cmd("cargo build", f"./{BOOTLOADER_DIR}")
    _run_cmd(
        f"cp ./target/x86_64-unknown-uefi/debug/bootloader.efi ./{OUTPUT_DIR}/{BOOTLOADER_FILE}"
    )


def _build_kernel():
    global is_kernel_test, test_kernel_path
    kernel_path = (
        test_kernel_path
        if is_kernel_test and test_kernel_path != ""
        else "./target/x86_64-kernel/debug/kernel"
    )

    _run_cmd("cargo build", f"./{KERNEL_DIR}")
    _run_cmd(f"cp {kernel_path} ./{OUTPUT_DIR}/{KERNEL_FILE}")


def build():
    global is_kernel_test

    _init()
    if not is_kernel_test:
        _build_apps()

    _build_cozette()
    # _build_qemu()
    _build_bootloader()
    _build_kernel()


def _build_apps():
    d = f"./{APPS_DIR}"
    dirs = [f for f in os.listdir(d) if os.path.isdir(os.path.join(d, f))]
    dirs.sort()
    dirs.remove(APPS_LIBC_DIR)

    for dir_name in dirs:
        pwd = f"{d}/{dir_name}"

        if os.path.exists(f"{pwd}/Makefile"):
            _run_cmd("make clean", dir=pwd)
            _run_cmd("make", dir=pwd)

    # copy apps dir to initramfs dir
    _run_cmd(f"rm -rf ./{INITRAMFS_DIR}/{APPS_DIR}")
    _run_cmd(f"mkdir -p ./{INITRAMFS_DIR}/{APPS_DIR}")
    _run_cmd(f"cp -r {d}/bin ./{INITRAMFS_DIR}/{APPS_DIR}/")

    # remove `target` directory
    _run_cmd(f'find ./{INITRAMFS_DIR} -type d -name "target" | xargs rm -rf')

    _build_doom()


def _make_initramfs():
    _run_cmd(
        f"dd if=/dev/zero of=./{OUTPUT_DIR}/{INITRAMFS_IMG_FILE} bs=1M count=128"
    )  # 128MiB
    _run_cmd(
        f'mkfs.fat -n "INITRAMFS" -F 32 -s 2 ./{OUTPUT_DIR}/{INITRAMFS_IMG_FILE}'
    )  # format for FAT32
    _run_cmd(f"sudo mount -o loop ./{OUTPUT_DIR}/{INITRAMFS_IMG_FILE} {MNT_DIR_PATH}")
    _run_cmd(f"sudo rm -rf {MNT_DIR_PATH}/*")  # clear initramfs
    _run_cmd(f"sudo cp -r ./{INITRAMFS_DIR}/* {MNT_DIR_PATH}/")
    _run_cmd("sleep 0.5")
    _run_cmd(f"sudo umount {MNT_DIR_PATH}")


def _make_img():
    _make_initramfs()
    _run_cmd(f"qemu-img create -f raw ./{OUTPUT_DIR}/{IMG_FILE} 200M")
    _run_cmd(
        f'mkfs.fat -n "MYOS" -F 32 -s 2 ./{OUTPUT_DIR}/{IMG_FILE}'
    )  # format for FAT32
    _run_cmd(f"sudo mount -o loop ./{OUTPUT_DIR}/{IMG_FILE} {MNT_DIR_PATH}")
    _run_cmd(f"sudo mkdir -p {MNT_DIR_PATH}/EFI/BOOT")
    _run_cmd(f"sudo mkdir -p {MNT_DIR_PATH}/EFI/myos")
    _run_cmd(
        f"sudo cp ./{OUTPUT_DIR}/{BOOTLOADER_FILE} {MNT_DIR_PATH}/EFI/BOOT/BOOTX64.EFI"
    )
    _run_cmd(f"sudo cp ./{OUTPUT_DIR}/{KERNEL_FILE} {MNT_DIR_PATH}/EFI/myos/kernel.elf")
    _run_cmd(
        f"sudo cp ./{OUTPUT_DIR}/{INITRAMFS_IMG_FILE} {MNT_DIR_PATH}/initramfs.img"
    )
    _run_cmd("sleep 0.5")
    _run_cmd(f"sudo umount {MNT_DIR_PATH}")


def make_iso():
    build()
    _make_img()
    _run_cmd(f"dd if=./{OUTPUT_DIR}/{IMG_FILE} of=./{OUTPUT_DIR}/{ISO_FILE} bs=1M")


def make_netdev():
    _run_cmd(f"sudo ip link add name {NETDEV_BR} type bridge")
    _run_cmd(f"sudo ip addr add {NETDEV_IP} dev {NETDEV_BR}")
    _run_cmd(f"sudo ip link set {NETDEV_BR} up")

    _run_cmd(f"sudo ip tuntap add {NETDEV_TAP} mode tap")
    _run_cmd(f"sudo ip link set {NETDEV_TAP} up")

    _run_cmd(f"sudo ip link set {NETDEV_TAP} master {NETDEV_BR}")


def del_netdev():
    _run_cmd(f"sudo ip link del {NETDEV_BR}")
    _run_cmd(f"sudo ip link del {NETDEV_TAP}")


def run():
    global is_kernel_test

    build()
    _make_img()
    # cmd = qemu_cmd() if is_kernel_test else _own_qemu_cmd()
    cmd = _qemu_cmd()

    _run_cmd(cmd, ignore_error=not is_kernel_test, check_qemu_exit_code=is_kernel_test)


def run_nographic():
    build()
    _make_img()
    _run_cmd(f"{_qemu_cmd()} -nographic", ignore_error=True)


def run_with_gdb():
    build()
    _make_img()
    _run_cmd(f"{_qemu_cmd()} -S")


def monitor():
    _run_cmd(f"telnet localhost {QEMU_MONITOR_PORT}")


def gdb():
    _run_cmd(f'gdb ./{OUTPUT_DIR}/{KERNEL_FILE} -ex "target remote :{QEMU_GDB_PORT}"')


def dump():
    build()
    _run_cmd(f"objdump -d ./{OUTPUT_DIR}/{KERNEL_FILE} > ./{DUMP_DIR}/dump_kernel.txt")
    _run_cmd(
        f"objdump -d ./{OUTPUT_DIR}/{BOOTLOADER_FILE} > ./{DUMP_DIR}/dump_bootloader.txt"
    )


def kernel_test_runner(kernel_path: str):
    global is_kernel_test, test_kernel_path
    os.chdir("../")
    is_kernel_test = True
    test_kernel_path = kernel_path
    run()


def clean():
    _run_cmd(f"rm -rf ./{OUTPUT_DIR}")
    _run_cmd(f"rm -rf ./{DUMP_DIR}")
    _run_cmd(f"rm -f ./{THIRD_PARTY_DIR}/{DOOM_WAD_FILE}")
    _run_cmd(f"rm -f ./{THIRD_PARTY_DIR}/{FONT_FILE}")
    _run_cmd(f"rm -f ./{THIRD_PARTY_DIR}/{COZETTE_BDF}")
    _run_cmd(f"rm -rf ./{THIRD_PARTY_DIR}/{DOOM_DIR}/build")
    _run_cmd(f"rm -rf ./{THIRD_PARTY_DIR}/{QEMU_DIR}/build")
    _run_cmd(f"make -C ./{THIRD_PARTY_DIR}/{DOOM_DIR} clean")
    _run_cmd(
        f"make -C ./{THIRD_PARTY_DIR}/{DOOM_DIR}/berkley-softfloat-3/build/Linux-x86_64-GCC clean"
    )
    _run_cmd("cargo clean")

    # clean apps
    apps_dir = f"./{APPS_DIR}"
    app_dirs = [
        f for f in os.listdir(apps_dir) if os.path.isdir(os.path.join(apps_dir, f))
    ]

    for dir_name in app_dirs:
        pwd = f"{apps_dir}/{dir_name}"

        if os.path.exists(f"{pwd}/Makefile"):
            _run_cmd("make clean", dir=pwd)
        else:
            _run_cmd("cargo clean", dir=pwd)

    _run_cmd(f"rm -rf ./{APPS_DIR}/bin")


TASKS = [
    build,
    make_iso,
    make_netdev,
    del_netdev,
    run,
    run_nographic,
    run_with_gdb,
    monitor,
    gdb,
    dump,
    clean,
]

if __name__ == "__main__":
    args = sys.argv

    if len(args) >= 2:
        if args[1] == "test" and len(args) >= 3:
            kernel_test_runner(args[2])
            exit(0)

        for task in TASKS:
            if task.__name__ == args[1]:
                task()
                exit(0)

        print("Invalid task name.")
    else:
        print(f"Usage: {list(map(lambda x: x.__name__, TASKS))}")

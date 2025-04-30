use common::kernel_config::KernelConfig;

#[derive(Debug)]
pub struct BootConfig<'a> {
    pub kernel_path: &'a str,
    pub initramfs_path: &'a str,
    pub resolution: (usize, usize),
}

impl Default for BootConfig<'_> {
    fn default() -> Self {
        Self {
            kernel_path: "\\EFI\\myos\\kernel.elf",
            initramfs_path: "initramfs.img",
            resolution: (800, 600),
        }
    }
}

pub const KERNEL_CONFIG: KernelConfig = KernelConfig {
    init_cwd_path: "/mnt/initramfs",
    init_app_exec_args: Some("/mnt/initramfs/apps/bin/sh /mnt/initramfs/apps/bin"),
    mouse_pointer_bmp_path: "/mnt/initramfs/sys/mouse_pointer.bmp",
};

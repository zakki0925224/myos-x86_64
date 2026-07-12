use crate::{
    arch::VirtualAddress,
    error::Result,
    fs::fat::{volume::FatVolume, Fat},
    kinfo,
};
use alloc::boxed::Box;
use common::kernel_config::KernelConfig;

pub mod fat;
pub mod file;
pub mod path;
pub mod vfs;

pub fn init(initramfs_virt_addr: VirtualAddress, kernel_config: &KernelConfig) -> Result<()> {
    vfs::init()?;
    kinfo!("fs: VFS initialized");

    let fat_volume = FatVolume::new(initramfs_virt_addr);
    let fat_fs = Fat::new(fat_volume);

    vfs::mount_fs(&"/mnt/initramfs".into(), Box::new(fat_fs))?;
    kinfo!("fs: Mounted initramfs to VFS");

    let dirname = kernel_config.init_cwd_path.into();
    vfs::chdir(&dirname)?;

    Ok(())
}

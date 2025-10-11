use super::{path::Path, vfs};
use crate::{debug::dwarf, error::Result, kerror, kinfo, task};
use common::elf::Elf64;

pub fn exec_elf(elf_path: &Path, args: &[&str], enable_debug: bool) -> Result<()> {
    let fd_num = vfs::open_file(elf_path, false)?;
    let elf_data = vfs::read_file(&fd_num)?;
    let elf64 = match Elf64::new(&elf_data) {
        Ok(e) => e,
        Err(err) => return Err(err.into()),
    };

    vfs::close_file(&fd_num)?;

    let dwarf = if enable_debug {
        match dwarf::parse(&elf64) {
            Ok(d) => Some(d),
            Err(err) => {
                kerror!("exec: Failed to parse DWARF: {:?}", err);
                None
            }
        }
    } else {
        None
    };

    let exit_code = task::exec_user_task(elf64, elf_path, args, dwarf)?;
    kinfo!("exec: Exited (code: {})", exit_code);

    Ok(())
}

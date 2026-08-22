use crate::{
    error::Result,
    fs::{
        path::Path,
        vfs::{self, FileDescriptorNumber},
    },
    task::TaskId,
};
use common::elf::Elf64;

pub fn exec_elf(
    elf_path: &Path,
    args: &[&str],
    pipe_fd: [Option<FileDescriptorNumber>; 3],
) -> Result<TaskId> {
    let fd_num = vfs::open_file(elf_path, false)?;
    let elf_data = vfs::read_file(fd_num, usize::MAX)?;
    let elf64 = Elf64::new(&elf_data)?;

    vfs::close_file(fd_num)?;

    // let dwarf = match dwarf::parse(&elf64) {
    //     Ok(d) => Some(d),
    //     Err(err) => {
    //         kerror!("exec: Failed to parse DWARF: {:?}", err);
    //         None
    //     }
    // };
    let dwarf = None;

    super::scheduler::spawn_user_task(elf64, elf_path, args, dwarf, pipe_fd)
}

use crate::{
    arch::{x86_64::context::ContextMode, VirtualAddress},
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{path::Path, vfs::FileDescriptorNumber},
    graphics::multi_layer::LayerId,
    kdebug,
    mem::bitmap::MemoryFrameInfo,
    task::{self, Task},
};
use alloc::{string::ToString, vec::Vec};
use common::elf::Elf64;

static mut TASK_SCHED: TaskScheduler = TaskScheduler::new();

struct TaskScheduler {
    kernel_task: Option<Task>,
    user_tasks: Vec<Task>,
    user_exit_status: Option<i32>,
}

impl TaskScheduler {
    const fn new() -> Self {
        Self {
            kernel_task: None,
            user_tasks: Vec::new(),
            user_exit_status: None,
        }
    }

    fn exec_user_task(
        &mut self,
        elf64: Elf64,
        path: &Path,
        args: &[&str],
        dwarf: Option<Dwarf>,
    ) -> Result<i32> {
        if self.kernel_task.is_none() {
            // stack is unused, because already allocated static area for kernel stack
            self.kernel_task = Some(Task::new(0, None, None, ContextMode::Kernel, None)?);
        }

        let is_user = !self.user_tasks.is_empty();
        if is_user {
            self.user_tasks.last().unwrap().unmap_virt_addr()?;
        }

        let user_task = Task::new(
            task::USER_TASK_STACK_SIZE,
            Some(elf64),
            Some(&[&[path.to_string().as_str()], args].concat()),
            ContextMode::User,
            dwarf,
        );

        let task = match user_task {
            Ok(task) => task,
            Err(e) => {
                if is_user {
                    self.user_tasks.last().unwrap().remap_virt_addr()?;
                }
                return Err(e);
            }
        };

        self.user_tasks.push(task);

        let is_user = self.user_tasks.len() > 1;
        let current_task = if is_user {
            self.user_tasks.get(self.user_tasks.len() - 2).unwrap()
        } else {
            self.kernel_task.as_ref().unwrap()
        };

        current_task.switch_to(self.user_tasks.last().unwrap());

        // returned
        drop(self.user_tasks.pop().unwrap());
        if let Some(task) = self.user_tasks.last() {
            task.remap_virt_addr()?;
        }

        // get exit status
        let exit_status = match self.user_exit_status {
            Some(s) => s,
            None => return Err(Error::Failed("User exit status was not found")),
        };
        self.user_exit_status = None;

        Ok(exit_status)
    }

    fn push_allocated_mem_frame_info_for_user_task(
        &mut self,
        mem_frame_info: MemoryFrameInfo,
    ) -> Result<()> {
        let user_task = self.user_tasks.iter_mut().last().unwrap();
        user_task
            .resource
            .allocated_mem_frame_info
            .push(mem_frame_info);

        Ok(())
    }

    fn get_memory_frame_size_by_virt_addr(
        &mut self,
        virt_addr: VirtualAddress,
    ) -> Result<Option<usize>> {
        let user_task = self.user_tasks.iter_mut().last().unwrap();

        for mem_frame_info in &user_task.resource.allocated_mem_frame_info {
            if mem_frame_info.frame_start_virt_addr()? == virt_addr {
                return Ok(Some(mem_frame_info.frame_size));
            }
        }

        Ok(None)
    }

    fn push_layer_id(&mut self, layer_id: LayerId) {
        let user_task = self.user_tasks.iter_mut().last().unwrap();
        user_task.resource.created_layer_ids.push(layer_id);
    }

    fn remove_layer_id(&mut self, layer_id: &LayerId) {
        let user_task = self.user_tasks.iter_mut().last().unwrap();

        user_task
            .resource
            .created_layer_ids
            .retain(|cwd| *cwd != *layer_id);
    }

    fn push_fd_num(&mut self, fd_num: FileDescriptorNumber) {
        let user_task = self.user_tasks.iter_mut().last().unwrap();
        user_task.resource.opend_fd_num.push(fd_num);
    }

    fn remove_fd_num(&mut self, fd_num: &FileDescriptorNumber) {
        let user_task = self.user_tasks.iter_mut().last().unwrap();

        user_task
            .resource
            .opend_fd_num
            .retain(|cfdn| *cfdn != *fd_num);
    }

    fn return_task(&mut self, exit_status: i32) {
        self.user_exit_status = Some(exit_status);

        let current_task = self.user_tasks.last().unwrap();

        let before_task;
        if let Some(before_task_i) = self.user_tasks.len().checked_sub(2) {
            before_task = self.user_tasks.get(before_task_i).unwrap();
        } else {
            before_task = self.kernel_task.as_ref().unwrap();
        }

        current_task.switch_to(before_task);
        unreachable!();
    }

    fn debug_user_task(&self) -> bool {
        kdebug!("===USER TASK INFO===");
        let user_task = self.user_tasks.last();
        if let Some(task) = user_task {
            task::debug_task(task);
            true
        } else {
            kdebug!("User task no available");
            false
        }
    }

    fn get_running_user_task_dwarf(&self) -> Option<Dwarf> {
        let user_task = self.user_tasks.last();
        if let Some(task) = user_task {
            task.dwarf.clone()
        } else {
            None
        }
    }
}

pub fn exec_user_task(
    elf64: Elf64,
    path: &Path,
    args: &[&str],
    dwarf: Option<Dwarf>,
) -> Result<i32> {
    unsafe { TASK_SCHED.exec_user_task(elf64, path, args, dwarf) }
}

pub fn push_allocated_mem_frame_info_for_user_task(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    unsafe { TASK_SCHED.push_allocated_mem_frame_info_for_user_task(mem_frame_info) }
}

pub fn get_memory_frame_size_by_virt_addr(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    unsafe { TASK_SCHED.get_memory_frame_size_by_virt_addr(virt_addr) }
}

pub fn push_layer_id(layer_id: LayerId) {
    unsafe { TASK_SCHED.push_layer_id(layer_id) }
}

pub fn remove_layer_id(layer_id: &LayerId) {
    unsafe { TASK_SCHED.remove_layer_id(layer_id) }
}

pub fn push_fd_num(fd_num: FileDescriptorNumber) {
    unsafe { TASK_SCHED.push_fd_num(fd_num) }
}

pub fn remove_fd_num(fd_num: &FileDescriptorNumber) {
    unsafe { TASK_SCHED.remove_fd_num(fd_num) }
}

pub fn return_task(exit_status: i32) {
    unsafe { TASK_SCHED.return_task(exit_status) }
}

pub fn debug_user_task() -> bool {
    unsafe { TASK_SCHED.debug_user_task() }
}

pub fn get_running_user_task_dwarf() -> Option<Dwarf> {
    unsafe { TASK_SCHED.get_running_user_task_dwarf() }
}

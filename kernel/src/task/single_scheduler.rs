use crate::{
    arch::{x86_64::context::ContextMode, VirtualAddress},
    debug::dwarf::Dwarf,
    error::{Error, Result},
    fs::{path::Path, vfs::FileDescriptorNumber},
    graphics::multi_layer::LayerId,
    kdebug,
    mem::bitmap::MemoryFrameInfo,
    task::{self, *},
};
use alloc::{string::ToString, vec::Vec};
use common::elf::Elf64;

static mut SINGLE_TASK_SCHED: SingleTaskScheduler = SingleTaskScheduler::new();

struct SingleTaskScheduler {
    kernel_task: Option<Task>,
    user_tasks: Vec<Task>,
    user_exit_status: Option<i32>,
}

impl SingleTaskScheduler {
    const fn new() -> Self {
        Self {
            kernel_task: None,
            user_tasks: Vec::new(),
            user_exit_status: None,
        }
    }

    fn current_user_task(&self) -> Option<&Task> {
        self.user_tasks.last()
    }

    fn current_user_task_mut(&mut self) -> Option<&mut Task> {
        self.user_tasks.last_mut()
    }

    fn prev_task(&self) -> Option<&Task> {
        if self.user_tasks.len() >= 2 {
            self.user_tasks.get(self.user_tasks.len() - 2)
        } else {
            self.kernel_task.as_ref()
        }
    }

    // temporarily pop the current user task, execute closure `f`, and push back if needed.
    fn with_popped_user_task<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(Task, &mut Self) -> R,
    {
        let task = self.user_tasks.pop().expect("No user task to pop");
        let result = f(task, self);
        result
    }

    fn exec_user_task(
        &mut self,
        elf64: Elf64,
        path: &Path,
        args: &[&str],
        dwarf: Option<Dwarf>,
    ) -> Result<i32> {
        if self.kernel_task.is_none() {
            self.kernel_task = Some(Task::new(0, None, None, ContextMode::Kernel, None)?);
        }

        let has_user = !self.user_tasks.is_empty();
        if has_user {
            self.current_user_task()
                .ok_or("No current user task")?
                .unmap_virt_addr()?;
        }

        let user_task = Task::new(
            task::USER_TASK_STACK_SIZE,
            Some(elf64),
            Some(&[&[path.to_string().as_str()], args].concat()),
            ContextMode::User,
            dwarf,
        )?;

        self.user_tasks.push(user_task);

        let current = self.prev_task().ok_or("No prev task")?;
        let new_task = self.current_user_task().ok_or("No current user task")?;
        current.switch_to(new_task);

        self.with_popped_user_task(|finished_task, sched| {
            drop(finished_task);
            if let Some(prev) = sched.current_user_task_mut() {
                prev.remap_virt_addr().unwrap();
            }
        });

        let exit_status = self
            .user_exit_status
            .take()
            .ok_or::<Error>("User exit status not found".into())?;

        Ok(exit_status)
    }

    fn push_allocated_mem_frame_info_for_user_task(
        &mut self,
        mem_frame_info: MemoryFrameInfo,
    ) -> Result<()> {
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;
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
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;

        for mem_frame_info in &user_task.resource.allocated_mem_frame_info {
            if mem_frame_info.frame_start_virt_addr()? == virt_addr {
                return Ok(Some(mem_frame_info.frame_size));
            }
        }

        Ok(None)
    }

    fn push_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;
        user_task.resource.created_layer_ids.push(layer_id);
        Ok(())
    }

    fn remove_layer_id(&mut self, layer_id: LayerId) -> Result<()> {
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;
        user_task
            .resource
            .created_layer_ids
            .retain(|cwd| *cwd != layer_id);
        Ok(())
    }

    fn push_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;
        user_task.resource.opend_fd_num.push(fd_num);
        Ok(())
    }

    fn remove_fd_num(&mut self, fd_num: FileDescriptorNumber) -> Result<()> {
        let user_task = self.current_user_task_mut().ok_or("No current user task")?;
        user_task.resource.opend_fd_num.retain(|f| *f != fd_num);
        Ok(())
    }

    fn return_task(&mut self, exit_status: i32) {
        self.user_exit_status = Some(exit_status);

        let current_task = self.current_user_task().unwrap();
        let before_task = self.prev_task().unwrap();

        current_task.switch_to(before_task);
        unreachable!();
    }

    fn debug_user_task(&self) -> bool {
        kdebug!("===USER TASK INFO===");
        if let Some(task) = self.current_user_task() {
            task::debug_task(task);
            true
        } else {
            kdebug!("User task no available");
            false
        }
    }

    fn get_running_user_task_dwarf(&self) -> Option<Dwarf> {
        self.current_user_task()?.dwarf.clone()
    }
}

pub fn exec_user_task(
    elf64: Elf64,
    path: &Path,
    args: &[&str],
    dwarf: Option<Dwarf>,
) -> Result<i32> {
    unsafe { SINGLE_TASK_SCHED.exec_user_task(elf64, path, args, dwarf) }
}

pub fn return_task(exit_status: i32) {
    unsafe { SINGLE_TASK_SCHED.return_task(exit_status) }
}

pub fn request(req: TaskRequest) -> Result<TaskResult> {
    match req {
        TaskRequest::PushLayerId(layer_id) => {
            unsafe { SINGLE_TASK_SCHED.push_layer_id(layer_id) }?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::RemoveLayerId(layer_id) => {
            unsafe { SINGLE_TASK_SCHED.remove_layer_id(layer_id) }?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::PushFileDescriptorNumber(fd_num) => {
            unsafe { SINGLE_TASK_SCHED.push_fd_num(fd_num) }?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::RemoveFileDescriptorNumber(fd_num) => {
            unsafe { SINGLE_TASK_SCHED.remove_fd_num(fd_num) }?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::PushMemory(mem_frame_info) => {
            unsafe {
                SINGLE_TASK_SCHED.push_allocated_mem_frame_info_for_user_task(mem_frame_info)
            }?;
            Ok(TaskResult::Ok)
        }
        TaskRequest::GetMemoryFrameSize(virt_addr) => {
            let size = unsafe { SINGLE_TASK_SCHED.get_memory_frame_size_by_virt_addr(virt_addr) }?;
            Ok(TaskResult::MemoryFrameSize(size))
        }
        TaskRequest::ExecuteDebugger => {
            let res = unsafe { SINGLE_TASK_SCHED.debug_user_task() };
            Ok(TaskResult::ExecuteDebugger(res))
        }
        TaskRequest::GetDwarf => {
            let dwarf = unsafe { SINGLE_TASK_SCHED.get_running_user_task_dwarf() };
            Ok(TaskResult::Dwarf(dwarf))
        }
    }
}

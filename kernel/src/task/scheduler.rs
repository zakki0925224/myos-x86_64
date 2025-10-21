use crate::{
    arch::{
        x86_64::{self, context::*},
        VirtualAddress,
    },
    debug::dwarf::Dwarf,
    error::*,
    fs::vfs::FileDescriptorNumber,
    graphics::multi_layer::LayerId,
    kdebug,
    mem::bitmap::MemoryFrameInfo,
    task::*,
};
use alloc::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
use common::elf::Elf64;

static mut TASK_SCHED: TaskScheduler = TaskScheduler::new();

struct TaskScheduler {
    tasks: BTreeMap<TaskId, Task>,
    task_order: VecDeque<TaskId>,
    current_idx: usize,
    kernel_task: Option<Task>,
}

impl TaskScheduler {
    const fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            task_order: VecDeque::new(),
            current_idx: 0,
            kernel_task: None,
        }
    }

    fn init_kernel_task(&mut self) -> Result<()> {
        if self.kernel_task.is_some() {
            return Err(Error::Failed("kernel task already initialized"));
        }

        self.kernel_task = Some(Task::new(0, None, None, ContextMode::Kernel, None)?);
        Ok(())
    }

    fn add_user_task(
        &mut self,
        elf64: Elf64,
        args: &[&str],
        mode: ContextMode,
        dwarf: Option<Dwarf>,
    ) -> Result<TaskId> {
        let task = Task::new(USER_TASK_STACK_SIZE, Some(elf64), Some(args), mode, dwarf)?;
        let tid = task.id.clone();

        self.tasks.insert(tid.clone(), task);
        self.task_order.push_back(tid.clone());

        kdebug!("sched: Added task: tid={}", tid.get());
        Ok(tid)
    }

    fn remove_user_task(&mut self, tid: &TaskId) {
        self.tasks.remove(tid);
        if let Some(pos) = self.task_order.iter().position(|t| t == tid) {
            self.task_order.remove(pos);
            if self.current_idx >= self.task_order.len() && !self.task_order.is_empty() {
                self.current_idx = 0;
            }
        }

        kdebug!("sched: Removed task: tid={}", tid.get());
    }

    fn current_user_task(&self) -> Result<&Task> {
        if self.task_order.is_empty() {
            return Err(Error::Failed("no current task"));
        }

        let tid = &self.task_order[self.current_idx % self.task_order.len()];
        self.tasks
            .get(tid)
            .ok_or(Error::Failed("current task not found"))
    }

    fn current_user_task_mut(&mut self) -> Result<&mut Task> {
        if self.task_order.is_empty() {
            return Err(Error::Failed("no current task"));
        }

        let tid = &self.task_order[self.current_idx % self.task_order.len()];
        self.tasks
            .get_mut(tid)
            .ok_or(Error::Failed("current task not found"))
    }

    fn next_user_task(&self) -> Result<&Task> {
        if self.task_order.is_empty() {
            return Err(Error::Failed("no tasks"));
        }

        let next_idx = (self.current_idx + 1) % self.task_order.len();
        let tid = &self.task_order[next_idx];
        self.tasks
            .get(tid)
            .ok_or(Error::Failed("next task not found"))
    }

    fn next_user_task_mut(&mut self) -> Result<&mut Task> {
        if self.task_order.is_empty() {
            return Err(Error::Failed("no tasks"));
        }

        let next_idx = (self.current_idx + 1) % self.task_order.len();
        let tid = &self.task_order[next_idx];
        self.tasks
            .get_mut(tid)
            .ok_or(Error::Failed("next task not found"))
    }

    fn kernel_task(&self) -> Result<&Task> {
        self.kernel_task
            .as_ref()
            .ok_or(Error::Failed("kernel task not initialized"))
    }

    fn start_first_user_task(&mut self) -> Result<()> {
        if self.task_order.is_empty() {
            return Err(Error::Failed("no tasks"));
        }

        let first_tid = &self.task_order[0];
        let first_task = self
            .tasks
            .get(first_tid)
            .ok_or(Error::Failed("first task not found"))?;

        first_task.remap_virt_addr()?;
        self.current_idx = 0;
        self.kernel_task()?.switch_to(first_task);

        Ok(())
    }

    fn poll(&mut self) -> Result<()> {
        if self.task_order.is_empty() {
            return Ok(());
        }
        assert!(self.kernel_task.is_some());

        kdebug!("sched: tasks: {:?}", self.tasks);
        kdebug!("sched: task_order: {:?}", self.task_order);

        let zombie_tids: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|(_, task)| task.state == TaskState::Zombie)
            .map(|(tid, _)| tid.clone())
            .collect();

        for tid in zombie_tids {
            self.reap_zombie(&tid);
        }

        let current_idx = self.current_idx % self.task_order.len();
        let next_idx = (current_idx + 1) % self.task_order.len();

        // self.tasks.len() == 1
        if current_idx == next_idx {
            return Ok(());
        }

        let current_tid = self.task_order[current_idx].clone();
        let next_tid = self.task_order[next_idx].clone();

        let current_task = self
            .tasks
            .get(&current_tid)
            .ok_or(Error::Failed("current task not found"))?;
        let next_task = self
            .tasks
            .get(&next_tid)
            .ok_or(Error::Failed("next task not found"))?;

        kdebug!(
            "sched: poll switching {} to {}",
            current_task.id.get(),
            next_task.id.get()
        );

        // unmap current
        current_task.unmap_virt_addr()?;

        // remap next
        next_task.remap_virt_addr()?;

        self.current_idx = next_idx;

        // switch context
        current_task.switch_to(next_task);

        Ok(())
    }

    fn exit_current_user_task(&mut self, exit_code: i32) -> ! {
        let current_tid = self.task_order[self.current_idx % self.task_order.len()].clone();

        kdebug!(
            "sched: Task {} exiting with code {}",
            current_tid.get(),
            exit_code
        );

        let current_context = if let Some(task) = self.tasks.get(&current_tid) {
            task.context.clone()
        } else {
            panic!("Current task not found");
        };

        // set to zombie task and unmap
        if let Some(task) = self.tasks.get_mut(&current_tid) {
            task.state = TaskState::Zombie;
            task.exit_code = Some(exit_code);
            task.unmap_virt_addr().unwrap();
        }

        // remove task from queue
        if let Some(pos) = self.task_order.iter().position(|t| t == &current_tid) {
            self.task_order.remove(pos);
            if !self.task_order.is_empty() && self.current_idx >= self.task_order.len() {
                self.current_idx = 0;
            }
        }

        // switch to next task or kernel
        if !self.task_order.is_empty() {
            let next_tid = &self.task_order[self.current_idx % self.task_order.len()];
            let next_task = self.tasks.get(next_tid).unwrap();

            kdebug!("sched: Switching to next task {}", next_task.id.get());
            next_task.remap_virt_addr().unwrap();

            current_context.switch_to(&next_task.context);
        } else {
            kdebug!("sched: Switching to kernel task");
            let kernel_task = self.kernel_task().unwrap();

            current_context.switch_to(&kernel_task.context);
        }

        unreachable!("exit_current_user_task: switch_to returned unexpectedly")
    }

    fn reap_zombie(&mut self, tid: &TaskId) -> Option<i32> {
        if let Some(task) = self.tasks.get(tid) {
            if task.state == TaskState::Zombie {
                let exit_code = task.exit_code;
                self.tasks.remove(tid);
                kdebug!("sched: Reaped zombie task {}", tid.get());
                return exit_code;
            }
        }
        None
    }
}

pub fn init() -> Result<()> {
    x86_64::disabled_int(|| unsafe { TASK_SCHED.init_kernel_task() })
}

pub fn add_user_task(elf64: Elf64, args: &[&str], dwarf: Option<Dwarf>) -> Result<TaskId> {
    x86_64::disabled_int(|| unsafe {
        TASK_SCHED.add_user_task(elf64, args, ContextMode::User, dwarf)
    })
}

pub fn start() -> Result<()> {
    x86_64::disabled_int(|| unsafe { TASK_SCHED.start_first_user_task() })
}

pub fn poll() -> Result<()> {
    x86_64::disabled_int(|| unsafe { TASK_SCHED.poll() })
}

pub fn exit_current_user_task(exit_code: i32) -> ! {
    unsafe { TASK_SCHED.exit_current_user_task(exit_code) }
}

pub fn push_allocated_mem_frame_info_for_user_task(mem_frame_info: MemoryFrameInfo) -> Result<()> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task_mut()?;
        user_task
            .resource
            .allocated_mem_frame_info
            .push(mem_frame_info);

        Result::Ok(())
    })
}

pub fn get_memory_frame_size_by_virt_addr(virt_addr: VirtualAddress) -> Result<Option<usize>> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task()?;
        for mem_frame_info in &user_task.resource.allocated_mem_frame_info {
            if mem_frame_info.frame_start_virt_addr()? == virt_addr {
                return Result::Ok(Some(mem_frame_info.frame_size));
            }
        }

        Result::Ok(None)
    })
}

pub fn push_layer_id(layer_id: LayerId) -> Result<()> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task_mut()?;
        user_task.resource.created_layer_ids.push(layer_id);

        Result::Ok(())
    })
}

pub fn remove_layer_id(layer_id: &LayerId) -> Result<()> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task_mut()?;
        user_task
            .resource
            .created_layer_ids
            .retain(|cwd| cwd.get() != layer_id.get());

        Result::Ok(())
    })
}

pub fn push_fd_num(fd_num: FileDescriptorNumber) -> Result<()> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task_mut()?;
        user_task.resource.opend_fd_num.push(fd_num);

        Result::Ok(())
    })
}

pub fn remove_fd_num(fd_num: &FileDescriptorNumber) -> Result<()> {
    x86_64::disabled_int(|| unsafe {
        let user_task = TASK_SCHED.current_user_task_mut()?;
        user_task
            .resource
            .opend_fd_num
            .retain(|cfdn| cfdn.get() != fd_num.get());

        Result::Ok(())
    })
}

pub fn debug_current_user_task() -> bool {
    kdebug!("===USER TASK INFO===");
    if let Ok(task) = x86_64::disabled_int(|| unsafe { TASK_SCHED.current_user_task() }) {
        super::debug_task(task);
        true
    } else {
        kdebug!("User task no available");
        false
    }
}

pub fn get_current_user_task_dwarf() -> Option<Dwarf> {
    x86_64::disabled_int(|| unsafe { TASK_SCHED.current_user_task() })
        .ok()?
        .dwarf
        .clone()
}

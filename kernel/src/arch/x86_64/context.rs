use super::{gdt::*, registers::*};
use common::boot_info::BootInfo;
use core::arch::{asm, naked_asm};

const KERNEL_STACK_SIZE: usize = 1024 * 1024;
static KERNEL_STACK: KernelStack = KernelStack::new();

#[repr(align(16))]
struct KernelStack([u8; KERNEL_STACK_SIZE]);

impl KernelStack {
    const fn new() -> Self {
        Self([0; KERNEL_STACK_SIZE])
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }
}

pub fn switch_kernel_stack(
    new_entry: extern "sysv64" fn(&BootInfo) -> !,
    boot_info: &BootInfo,
) -> ! {
    unsafe {
        asm!(
            "mov rdi, {}",
            "mov rsp, {}",
            "call {}",
            in(reg) boot_info,
            in(reg) KERNEL_STACK.as_ptr() as u64 + KERNEL_STACK.len() as u64,
            in(reg) new_entry
        );
    }
    unreachable!();
}

// software context switch
#[unsafe(naked)]
extern "sysv64" fn switch_context(next_ctx: &Context, current_ctx: &Context) {
    naked_asm!(
        // save context
        "pushfq",
        "pop qword ptr [rsi + 0x10]", // rflags
        "mov [rsi + 0x20], cs",
        "mov [rsi + 0x28], ss",
        "mov [rsi + 0x30], fs",
        "mov [rsi + 0x38], gs",
        "mov [rsi + 0x40], rax",
        "mov [rsi + 0x48], rbx",
        "mov [rsi + 0x50], rcx",
        "mov [rsi + 0x58], rdx",
        "mov [rsi + 0x60], rdi",
        "mov [rsi + 0x68], rsi",
        "lea rax, [rsp + 0x08]", // + stack frame offset
        "mov [rsi + 0x70], rax", // rsp
        "mov [rsi + 0x78], rbp",
        "mov rax, cr3",          // use already saved register
        "mov [rsi + 0x00], rax", // cr3
        "mov rax, [rsp]",
        "mov [rsi + 0x08], rax", // rip
        "mov [rsi + 0x80], r8",
        "mov [rsi + 0x88], r9",
        "mov [rsi + 0x90], r10",
        "mov [rsi + 0x98], r11",
        "mov [rsi + 0xa0], r12",
        "mov [rsi + 0xa8], r13",
        "mov [rsi + 0xb0], r14",
        "mov [rsi + 0xb8], r15",
        "fxsave64 [rsi + 0xc0]", // fpu_context
        // stack frame
        "push qword ptr [rdi + 0x28]", // ss
        "push qword ptr [rdi + 0x70]", // rsp
        "push qword ptr [rdi + 0x10]", // rflags
        "push qword ptr [rdi + 0x20]", // cs
        "push qword ptr [rdi + 0x08]", // rip
        // restore context
        "fxrstor64 [rdi + 0xc0]", // fpu_context
        "mov rax, [rdi + 0x00]",
        "mov cr3, rax", // cr3
        "mov rax, [rdi + 0x30]",
        "mov fs, ax", // fs
        "mov rax, [rdi + 0x38]",
        "mov gs, ax", // gs
        "mov rax, [rdi + 0x40]",
        "mov rbx, [rdi + 0x48]",
        "mov rcx, [rdi + 0x50]",
        "mov rdx, [rdi + 0x58]",
        "mov rsi, [rdi + 0x68]",
        "mov rbp, [rdi + 0x78]",
        "mov r8, [rdi + 0x80]",
        "mov r9, [rdi + 0x88]",
        "mov r10, [rdi + 0x90]",
        "mov r11, [rdi + 0x98]",
        "mov r12, [rdi + 0xa0]",
        "mov r13, [rdi + 0xa8]",
        "mov r14, [rdi + 0xb0]",
        "mov r15, [rdi + 0xb8]",
        "mov rdi, [rdi + 0x60]",
        "iretq"
    );
}

#[derive(PartialEq, Eq)]
pub enum ContextMode {
    Kernel,
    User,
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
// 16 byte alignment for fxsave64 and fxstor64 instructions
pub struct Context {
    /* + 0x00 */ pub cr3: u64,
    /* + 0x08 */ pub rip: u64,
    /* + 0x10 */ pub rflags: Rflags,
    /* + 0x18 */ reserved: u64,
    /* + 0x20 */ pub cs: u64,
    /* + 0x28 */ pub ss: u64,
    /* + 0x30 */ pub fs: u64,
    /* + 0x38 */ pub gs: u64,
    /* + 0x40 */ pub rax: u64,
    /* + 0x48 */ pub rbx: u64,
    /* + 0x50 */ pub rcx: u64,
    /* + 0x58 */ pub rdx: u64,
    /* + 0x60 */ pub rdi: u64,
    /* + 0x68 */ pub rsi: u64,
    /* + 0x70 */ pub rsp: u64,
    /* + 0x78 */ pub rbp: u64,
    /* + 0x80 */ pub r8: u64,
    /* + 0x88 */ pub r9: u64,
    /* + 0x90 */ pub r10: u64,
    /* + 0x98 */ pub r11: u64,
    /* + 0xa0 */ pub r12: u64,
    /* + 0xa8 */ pub r13: u64,
    /* + 0xb0 */ pub r14: u64,
    /* + 0xb8 */ pub r15: u64,
    /* + 0xc0 */ pub fpu_context: [u8; 512],
}

impl Context {
    pub const fn new() -> Self {
        Self {
            cr3: 0,
            rip: 0,
            rflags: Rflags::default(),
            reserved: 0,
            cs: 0,
            ss: 0,
            fs: 0,
            gs: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rdi: 0,
            rsi: 0,
            rsp: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            fpu_context: [0; 512],
        }
    }

    pub fn init(
        &mut self,
        rip: u64,
        rdi: u64,
        rsi: u64,
        rsp: u64,
        mode: ContextMode,
        trap_flag: bool,
    ) {
        let (cs, ss) = match mode {
            ContextMode::Kernel => (KERNEL_MODE_CS_VALUE, KERNEL_MODE_SS_VALUE),
            ContextMode::User => (USER_MODE_CS_VALUE, USER_MODE_SS_VALUE),
        };

        self.rip = rip;
        self.rdi = rdi;
        self.rsi = rsi;
        self.rsp = rsp;
        self.rbp = rsp;
        self.cr3 = Cr3::read().raw();

        self.rflags = Rflags::default();
        self.rflags.set_if_(true); // enable interrupts
        self.rflags.set_tf(trap_flag);

        self.cs = cs as u64;
        self.ss = ss as u64;
    }

    #[inline(always)]
    pub fn switch_to(&self, next_ctx: &Context) {
        switch_context(next_ctx, self);
    }
}

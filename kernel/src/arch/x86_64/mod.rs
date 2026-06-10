use crate::arch::x86_64::registers::{Register, Rflags};
use core::arch::asm;

pub mod acpi;
pub mod apic;
pub mod context;
pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod registers;
pub mod tsc;
pub mod tss;

#[repr(C, packed(2))]
#[derive(Debug, Default)]
pub struct DescriptorTableArgs {
    pub limit: u16,
    pub base: u64,
}

#[inline(always)]
pub fn stihlt() {
    unsafe { asm!("sti", "hlt", options(nomem, nostack)) }
}

#[inline(always)]
pub fn sti() {
    unsafe { asm!("sti", options(nomem, nostack)) }
}

#[inline(always)]
pub fn cli() {
    unsafe { asm!("cli", options(nomem, nostack)) }
}

pub fn disabled_int<F: FnMut() -> R, R>(mut func: F) -> R {
    let rflags = Rflags::read();
    cli();
    let func_res = func();

    if rflags.if_() {
        sti();
    }

    func_res
}

#[inline(always)]
pub fn int3() {
    unsafe { asm!("int3", options(nomem, nostack)) }
}

#[inline(always)]
pub fn out8(port: u16, data: u8) {
    unsafe {
        asm!(
            "out dx, al",
            in("dx") port,
            in("al") data,
            options(nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn in8(port: u16) -> u8 {
    let data: u8;
    unsafe {
        asm!(
            "in al, dx",
            out("al") data,
            in("dx") port,
            options(nomem, nostack)
        );
    }
    data
}

#[inline(always)]
pub fn out16(port: u16, data: u16) {
    unsafe {
        asm!(
            "out dx, ax",
            in("dx") port,
            in("ax") data,
            options(nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn in16(port: u16) -> u16 {
    let data: u16;
    unsafe {
        asm!(
            "in ax, dx",
            out("ax") data,
            in("dx") port,
            options(nomem, nostack)
        );
    }
    data
}

#[inline(always)]
pub fn out32(port: u32, data: u32) {
    unsafe {
        asm!(
            "out dx, eax",
            in("edx") port,
            in("eax") data,
            options(nomem, nostack)
        );
    }
}

#[inline(always)]
pub fn in32(port: u32) -> u32 {
    let data: u32;
    unsafe {
        asm!(
            "in eax, dx",
            out("eax") data,
            in("edx") port,
            options(nomem, nostack)
        );
    }
    data
}

#[inline(always)]
pub fn lidt(desc_table_args: &DescriptorTableArgs) {
    unsafe {
        asm!("lidt [{}]", in(reg) desc_table_args, options(nomem, nostack));
    }
}

#[inline(always)]
pub fn lgdt(desc_table_args: &DescriptorTableArgs) {
    unsafe {
        asm!("lgdt [{}]", in(reg) desc_table_args, options(nomem, nostack));
    }
}

#[inline(always)]
pub fn ltr(sel: u16) {
    unsafe {
        asm!("ltr cx", in("cx") sel, options(nomem, nostack));
    }
}

#[inline(always)]
pub fn read_msr(addr: u32) -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!("rdmsr", in("ecx") addr, out("eax") low, out("edx") high, options(nomem, nostack));
    }

    ((high as u64) << 32) | (low as u64)
}

#[inline(always)]
pub fn write_msr(addr: u32, value: u64) {
    let low = value as u32;
    let high = (value >> 32) as u32;

    unsafe {
        asm!("wrmsr", in("ecx") addr, in("eax") low, in("edx") high, options(nomem, nostack));
    }
}

#[inline(always)]
pub fn read_xcr0() -> u64 {
    let value;
    unsafe {
        asm!("xgetbv", out("rax") value, options(nomem, nostack));
    }
    value
}

#[inline(always)]
pub fn write_xcr0(value: u64) {
    unsafe {
        asm!("xsetbv", in("rax") value, options(nomem, nostack));
    }
}

#[inline(always)]
pub fn rdtsc() -> u64 {
    let low: u32;
    let high: u32;

    unsafe {
        asm!("rdtsc", out("eax") low, out("edx") high, options(nomem, nostack));
    }

    ((high as u64) << 32) | (low as u64)
}

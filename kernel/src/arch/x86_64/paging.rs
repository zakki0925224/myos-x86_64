use crate::{
    arch::{
        x86_64::registers::{Cr3, Register},
        VirtualAddress,
    },
    error::Result,
    mem::{
        bitmap::{self, MemoryFrame},
        paging::PageError,
    },
};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

static KERNEL_PML4_PHYS: AtomicU64 = AtomicU64::new(0);

const PAGE_TABLE_ENTRY_LEN: usize = 512;
pub const PAGE_SIZE: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
#[repr(u8)]
pub enum ReadWrite {
    Read = 0,
    Write = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
#[repr(u8)]
pub enum EntryMode {
    Supervisor = 0,
    User = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageWriteThroughLevel {
    WriteBack = 0,
    WriteThrough = 1,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct PageTableEntry(u64);

impl core::fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PageTableEntry")
            .field("0", &format_args!("{:#x}", self.0))
            .field("p", &self.p())
            .field("rw", &self.rw())
            .field("us", &self.us())
            .field("pcd", &self.pcd())
            .field("a", &self.accessed())
            .field("d", &self.dirty())
            .field("page_size", &self.page_size())
            .field("addr", &format_args!("{:#x}", self.addr()))
            .field("xd", &self.exec_disable())
            .finish()
    }
}

impl PageTableEntry {
    const ADDR_MASK: u64 = 0x0000_007f_ffff_f000;

    pub fn set_p(&mut self, value: bool) {
        self.0 = (self.0 & !0x1) | (value as u64);
    }

    pub fn p(&self) -> bool {
        (self.0 & 0x1) != 0
    }

    pub fn set_rw(&mut self, rw: ReadWrite) {
        let rw = rw as u64;
        self.0 = (self.0 & !0x2) | (rw << 1);
    }

    pub fn rw(&self) -> ReadWrite {
        match (self.0 & 0x2) != 0 {
            true => ReadWrite::Write,
            false => ReadWrite::Read,
        }
    }

    pub fn set_us(&mut self, us: EntryMode) {
        let us = us as u64;
        self.0 = (self.0 & !0x4) | (us << 2);
    }

    pub fn us(&self) -> EntryMode {
        match (self.0 & 0x4) != 0 {
            true => EntryMode::User,
            false => EntryMode::Supervisor,
        }
    }

    pub fn set_pwt(&mut self, pwt: PageWriteThroughLevel) {
        let pwt = pwt as u64;
        self.0 = (self.0 & !0x8) | (pwt << 3);
    }

    pub fn pwt(&self) -> PageWriteThroughLevel {
        match (self.0 & 0x8) != 0 {
            true => PageWriteThroughLevel::WriteThrough,
            false => PageWriteThroughLevel::WriteBack,
        }
    }

    // page cache disable
    pub fn set_pcd(&mut self, pcd: bool) {
        self.0 = (self.0 & !0x10) | ((pcd as u64) << 4);
    }

    pub fn pcd(&self) -> bool {
        (self.0 & 0x10) != 0
    }

    pub fn accessed(&self) -> bool {
        (self.0 & 0x20) != 0
    }

    pub fn dirty(&self) -> bool {
        (self.0 & 0x40) != 0
    }

    pub fn page_size(&self) -> bool {
        (self.0 & 0x80) != 0
    }

    pub fn set_addr(&mut self, addr: u64) {
        self.0 = (self.0 & !Self::ADDR_MASK) | (addr & Self::ADDR_MASK);
    }

    pub fn addr(&self) -> u64 {
        self.0 & Self::ADDR_MASK
    }

    pub fn exec_disable(&self) -> bool {
        (self.0 & (1 << 63)) != 0
    }

    pub unsafe fn page_table(&self) -> Option<&PageTable> {
        if self.page_size() {
            return None;
        }

        let ptr = self.addr() as *const PageTable;
        Some(&*ptr)
    }

    pub unsafe fn page_table_mut(&self) -> Option<&mut PageTable> {
        if self.page_size() {
            return None;
        }

        let ptr_mut = self.addr() as *mut PageTable;
        Some(&mut *ptr_mut)
    }

    pub fn set_entry(
        &mut self,
        addr: u64,
        rw: ReadWrite,
        us: EntryMode,
        pwt: PageWriteThroughLevel,
        pcd: bool,
    ) {
        self.set_p(true);
        self.set_rw(rw);
        self.set_us(us);
        self.set_pwt(pwt);
        self.set_pcd(pcd);
        self.set_addr(addr);
    }
}

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; PAGE_TABLE_ENTRY_LEN],
}

unsafe fn ensure_pte<'a>(
    table: &'a mut PageTable,
    index: usize,
    rw: ReadWrite,
    us: EntryMode,
    pwt: PageWriteThroughLevel,
    pcd: bool,
    alloc_frame: &mut dyn FnMut() -> Result<MemoryFrame>,
) -> Result<(Option<MemoryFrame>, &'a mut PageTableEntry)> {
    let entry = &mut table.entries[index];
    let mut frame = None;

    if !entry.p() {
        let mem_frame = alloc_frame()?;
        mem_frame.zero_out()?;
        entry.set_entry(mem_frame.frame_start_phys_addr(), rw, us, pwt, pcd);
        frame = Some(mem_frame);
    }

    if entry.rw() < rw {
        entry.set_rw(rw);
    }
    if entry.us() < us {
        entry.set_us(us);
    }

    Ok((frame, entry))
}

fn map(
    pml4_table: &mut PageTable,
    start: VirtualAddress,
    end: VirtualAddress,
    phys_addr: u64,
    rw: ReadWrite,
    us: EntryMode,
    pwt: PageWriteThroughLevel,
    pcd: bool,
    alloc_frame: &mut dyn FnMut() -> Result<MemoryFrame>,
    mut allocated_frame: impl FnMut(MemoryFrame),
) -> Result<()> {
    if start.get() % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(start.get()).into());
    }

    if end.get() % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(end.get()).into());
    }

    if phys_addr % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(phys_addr).into());
    }

    for i in (start.get()..end.get()).step_by(PAGE_SIZE) {
        let virt_addr: VirtualAddress = i.into();
        let indicies = [
            virt_addr.pml4_entry_index(),
            virt_addr.pml3_entry_index(),
            virt_addr.pml2_entry_index(),
        ];

        let mut table_ptr = pml4_table as *mut PageTable;
        for &index in &indicies {
            let (frame, pte) =
                unsafe { ensure_pte(&mut *table_ptr, index, rw, us, pwt, pcd, alloc_frame)? };

            if let Some(f) = frame {
                allocated_frame(f);
            }

            table_ptr = unsafe { pte.page_table_mut() }.unwrap();
        }

        let pml1_table = unsafe { &mut *table_ptr };
        let pte = &mut pml1_table.entries[virt_addr.pml1_entry_index()];
        pte.set_entry(phys_addr + (i - start.get()), rw, us, pwt, pcd);
    }

    Ok(())
}

#[derive(Debug)]
pub struct UserPageTable {
    pml4_frame: Option<MemoryFrame>,
    allocated_frames: Vec<MemoryFrame>,
}

unsafe fn clone_table_entries(src: *const PageTable) -> Result<MemoryFrame> {
    let frame = bitmap::alloc_mem_frame(1)?;
    let dst = frame.frame_start_virt_addr().as_ptr_mut::<PageTable>();
    (*dst).entries = (*src).entries;
    Ok(frame)
}

unsafe fn ensure_task_table(
    owned: &mut Vec<MemoryFrame>,
    pte: &mut PageTableEntry,
    rw: ReadWrite,
    pwt: PageWriteThroughLevel,
    pcd: bool,
) -> Result<()> {
    if !pte.p() {
        let frame = bitmap::alloc_mem_frame(1)?;
        frame.zero_out()?;
        pte.set_entry(frame.frame_start_phys_addr(), rw, EntryMode::User, pwt, pcd);
        owned.push(frame);
    } else {
        let phys = pte.addr();
        let is_owned = owned.iter().any(|f| f.frame_start_phys_addr() == phys);
        if !is_owned {
            let frame = clone_table_entries(phys as *const PageTable)?;
            pte.set_addr(frame.frame_start_phys_addr());
            owned.push(frame);
        }
        if pte.rw() < rw {
            pte.set_rw(rw);
        }
        if pte.us() < EntryMode::User {
            pte.set_us(EntryMode::User);
        }
    }
    Ok(())
}

impl Drop for UserPageTable {
    fn drop(&mut self) {
        for frame in self.allocated_frames.drain(..) {
            bitmap::dealloc_mem_frame(frame).unwrap();
        }

        if let Some(frame) = self.pml4_frame.take() {
            bitmap::dealloc_mem_frame(frame).unwrap();
        }
    }
}

impl UserPageTable {
    pub fn new() -> Result<Self> {
        let pml4_frame = bitmap::alloc_mem_frame(1)?;
        pml4_frame.zero_out()?;

        Ok(Self {
            pml4_frame: Some(pml4_frame),
            allocated_frames: Vec::new(),
        })
    }

    pub fn new_cloned_from_kernel() -> Result<Self> {
        let this = Self::new()?;

        unsafe {
            let kernel_pml4 = &*kernel_page_table();
            let user_pml4 = this
                .pml4_frame
                .as_ref()
                .unwrap()
                .frame_start_virt_addr()
                .as_ptr_mut::<PageTable>();

            (*user_pml4).entries = kernel_pml4.entries;
        }

        Ok(this)
    }

    pub fn pml4_phys_addr(&self) -> u64 {
        self.pml4_frame.as_ref().unwrap().frame_start_phys_addr()
    }

    pub unsafe fn as_page_table(&self) -> &PageTable {
        &*(self.pml4_phys_addr() as *const PageTable)
    }

    pub unsafe fn activate(&self) {
        let mut cr3 = Cr3::read();
        cr3.set_raw(self.pml4_phys_addr());
        cr3.write();
    }

    pub unsafe fn unmap(&mut self, start: VirtualAddress, end: VirtualAddress) {
        let pml4 = self
            .pml4_frame
            .as_ref()
            .unwrap()
            .frame_start_virt_addr()
            .as_ptr_mut::<PageTable>();

        for i in (start.get()..end.get()).step_by(PAGE_SIZE) {
            let virt = VirtualAddress::new(i);

            let pml4e = &(*pml4).entries[virt.pml4_entry_index()];
            if !pml4e.p() {
                continue;
            }
            let pml3 = pml4e.addr() as *mut PageTable;

            let pml3e = &(*pml3).entries[virt.pml3_entry_index()];
            if !pml3e.p() {
                continue;
            }
            let pml2 = pml3e.addr() as *mut PageTable;

            let pml2e = &(*pml2).entries[virt.pml2_entry_index()];
            if !pml2e.p() {
                continue;
            }
            let pml1 = pml2e.addr() as *mut PageTable;

            (*pml1).entries[virt.pml1_entry_index()].0 = 0;
            core::arch::asm!("invlpg [{0}]", in(reg) i, options(nostack));
        }
    }

    pub fn map(
        &mut self,
        start: VirtualAddress,
        end: VirtualAddress,
        phys_addr: u64,
        rw: ReadWrite,
        pwt: PageWriteThroughLevel,
        pcd: bool,
    ) -> Result<()> {
        let pml4_ptr: *mut PageTable = self
            .pml4_frame
            .as_ref()
            .unwrap()
            .frame_start_virt_addr()
            .as_ptr_mut();

        for i in (start.get()..end.get()).step_by(PAGE_SIZE) {
            let virt = VirtualAddress::new(i);
            let page_phys = phys_addr + (i - start.get());

            unsafe {
                let pml4e = &mut (*pml4_ptr).entries[virt.pml4_entry_index()];
                ensure_task_table(&mut self.allocated_frames, pml4e, rw, pwt, pcd)?;

                let pml3_ptr = pml4e.addr() as *mut PageTable;
                let pml3e = &mut (*pml3_ptr).entries[virt.pml3_entry_index()];
                ensure_task_table(&mut self.allocated_frames, pml3e, rw, pwt, pcd)?;

                let pml2_ptr = pml3e.addr() as *mut PageTable;
                let pml2e = &mut (*pml2_ptr).entries[virt.pml2_entry_index()];
                ensure_task_table(&mut self.allocated_frames, pml2e, rw, pwt, pcd)?;

                let pml1_ptr = pml2e.addr() as *mut PageTable;
                (*pml1_ptr).entries[virt.pml1_entry_index()].set_entry(
                    page_phys,
                    rw,
                    EntryMode::User,
                    pwt,
                    pcd,
                );
            }
        }

        Ok(())
    }
}

pub unsafe fn kernel_page_table() -> *const PageTable {
    KERNEL_PML4_PHYS.load(Ordering::Acquire) as *const PageTable
}

unsafe fn kernel_page_table_mut() -> *mut PageTable {
    KERNEL_PML4_PHYS.load(Ordering::Acquire) as *mut PageTable
}

pub fn kernel_init(start: VirtualAddress, end: VirtualAddress) -> Result<()> {
    let mut pml4_frame = bitmap::alloc_mem_frame(1)?;
    pml4_frame.zero_out()?;

    let pml4_table_ptr: *mut PageTable = pml4_frame.frame_start_virt_addr().as_ptr_mut();
    map(
        unsafe { &mut *pml4_table_ptr },
        start,
        end,
        start.get(),
        ReadWrite::Write,
        EntryMode::Supervisor,
        PageWriteThroughLevel::WriteBack,
        false,
        &mut || bitmap::alloc_mem_frame(1),
        |mut frame| frame.leak(),
    )?;

    let pml4_phys = pml4_frame.frame_start_phys_addr();
    let mut cr3 = Cr3::read();
    cr3.set_raw(pml4_phys);
    cr3.write();
    KERNEL_PML4_PHYS.store(pml4_phys, Ordering::Release);
    pml4_frame.leak();

    Ok(())
}

pub unsafe fn kernel_map(
    start: VirtualAddress,
    end: VirtualAddress,
    rw: ReadWrite,
    pwt: PageWriteThroughLevel,
    pcd: bool,
) -> Result<()> {
    map(
        &mut *kernel_page_table_mut(),
        start,
        end,
        start.get(),
        rw,
        EntryMode::Supervisor,
        pwt,
        pcd,
        &mut || bitmap::alloc_mem_frame(1),
        |mut frame| frame.leak(),
    )
}

pub unsafe fn kernel_remap(
    start: VirtualAddress,
    end: VirtualAddress,
    rw: ReadWrite,
    pwt: PageWriteThroughLevel,
    pcd: bool,
) -> Result<()> {
    remap(
        kernel_page_table_mut(),
        start,
        end,
        start.get(),
        rw,
        EntryMode::Supervisor,
        pwt,
        pcd,
    )
}

pub unsafe fn lookup_pte(
    pml4_table: &PageTable,
    virt_addr: VirtualAddress,
) -> Option<&PageTableEntry> {
    let pte = &pml4_table.entries[virt_addr.pml4_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &pte.page_table()?.entries[virt_addr.pml3_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &pte.page_table()?.entries[virt_addr.pml2_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &pte.page_table()?.entries[virt_addr.pml1_entry_index()];
    if !pte.p() {
        return None;
    }

    Some(pte)
}

unsafe fn lookup_pte_mut(
    pml4_table: &mut PageTable,
    virt_addr: VirtualAddress,
) -> Option<&mut PageTableEntry> {
    let pte = &pml4_table.entries[virt_addr.pml4_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &pte.page_table()?.entries[virt_addr.pml3_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &pte.page_table()?.entries[virt_addr.pml2_entry_index()];
    if !pte.p() {
        return None;
    }

    let pte = &mut pte.page_table_mut()?.entries[virt_addr.pml1_entry_index()];
    if !pte.p() {
        return None;
    }

    Some(pte)
}

pub unsafe fn calc_phys_addr(pml4_table: &PageTable, virt_addr: VirtualAddress) -> Option<u64> {
    let pte = lookup_pte(pml4_table, virt_addr)?;
    Some(pte.addr() | virt_addr.get() & 0xfff)
}

pub unsafe fn remap(
    pml4_table: *mut PageTable,
    start: VirtualAddress,
    end: VirtualAddress,
    phys_addr: u64,
    rw: ReadWrite,
    us: EntryMode,
    pwt: PageWriteThroughLevel,
    pcd: bool,
) -> Result<()> {
    if start.get() % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(start.get()).into());
    }

    if end.get() % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(end.get()).into());
    }

    if phys_addr % PAGE_SIZE as u64 != 0 {
        return Err(PageError::AddressNotAlignedByPageSize(phys_addr).into());
    }

    let pml4_table = unsafe { &mut *pml4_table };

    for i in (start.get()..end.get()).step_by(PAGE_SIZE) {
        let virt_addr: VirtualAddress = i.into();

        let pte_mut = lookup_pte_mut(pml4_table, virt_addr).ok_or(PageError::PageNotMapped)?;
        pte_mut.set_rw(rw);
        pte_mut.set_us(us);
        pte_mut.set_pwt(pwt);
        pte_mut.set_pcd(pcd);
        pte_mut.set_addr(phys_addr + (i - start.get()));
    }

    Ok(())
}

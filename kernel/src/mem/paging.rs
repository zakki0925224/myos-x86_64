use super::bitmap::MemoryFrameInfo;
use crate::{
    arch::{
        addr::*,
        register::{control::*, Register},
    },
    error::Result,
    info,
    mem::bitmap,
};

const PAGE_TABLE_ENTRY_LEN: usize = 512;
pub const PAGE_SIZE: usize = 4096;
static mut PAGE_MAN: PageManager = PageManager::new();

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
        let fmt = format!(
            "PageTableEntry(0x{:x}) {{ p: {}, rw: {:?}, us: {:?}, pcd: {}, a: {}, d: {}, page_size: {}, addr: 0x{:x}, xd: {} }}",
            self.0,
            self.p(),
            self.rw(),
            self.us(),
            self.pcd(),
            self.accessed(),
            self.dirty(),
            self.page_size(),
            self.addr(),
            self.exec_disable()
        );
        write!(f, "{}", fmt)
    }
}

impl PageTableEntry {
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
    pub fn set_pcd(&mut self, value: bool) {
        self.0 = (self.0 & !0x10) | ((value as u64) << 4);
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
        self.0 = (self.0 & 0xfff) | (addr & !0xfff);
    }

    pub fn addr(&self) -> u64 {
        self.0 & !0xfff
    }

    pub fn exec_disable(&self) -> bool {
        (self.0 & (1 << 63)) != 0
    }

    pub unsafe fn page_table(&self) -> Option<&mut PageTable> {
        match self.page_size() {
            true => None,
            false => {
                let ptr = self.addr() as *mut PageTable;
                Some(&mut *ptr)
            }
        }
    }

    pub fn set_entry(
        &mut self,
        addr: u64,
        rw: ReadWrite,
        mode: EntryMode,
        write_through_level: PageWriteThroughLevel,
        disable_cache: bool,
    ) {
        self.set_p(true);
        self.set_rw(rw);
        self.set_us(mode);
        self.set_pwt(write_through_level);
        self.set_pcd(disable_cache);
        self.set_addr(addr);
    }
}

#[derive(Debug)]
#[repr(C, align(4096))]
pub struct PageTable {
    pub entries: [PageTableEntry; PAGE_TABLE_ENTRY_LEN],
}

#[derive(Debug, Clone, Copy)]
pub struct MappingInfo {
    pub start: VirtualAddress,
    pub end: VirtualAddress,
    pub phys_addr: PhysicalAddress,
    pub rw: ReadWrite,
    pub us: EntryMode,
    pub pwt: PageWriteThroughLevel,
    pub pcd: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageManagerError {
    VirtualAddressNotMappedError(VirtualAddress),
    VirtualAddressNotAllowedToMapError(VirtualAddress),
    VirtualAddressNotAlignedByPageSizeError(VirtualAddress),
    PhysicalAddressNotMappedError(PhysicalAddress),
}

#[derive(Debug)]
pub struct PageManager;

impl PageManager {
    pub const fn new() -> Self {
        Self
    }

    pub unsafe fn calc_virt_addr(&self, phys_addr: PhysicalAddress) -> Result<VirtualAddress> {
        let pml4_table = self.pml4_table();
        for pml4_i in 0..PAGE_TABLE_ENTRY_LEN {
            let pml4_entry = &pml4_table.entries[pml4_i];
            if !pml4_entry.p() {
                continue;
            }

            let pml3_table = match pml4_entry.page_table() {
                Some(table) => table,
                None => continue,
            };
            for pml3_i in 0..PAGE_TABLE_ENTRY_LEN {
                let pml3_entry = &pml3_table.entries[pml3_i];
                if !pml3_entry.p() {
                    continue;
                }

                let pml2_table = match pml3_entry.page_table() {
                    Some(table) => table,
                    None => {
                        let virt_addr_raw = pml3_entry.addr();
                        if (phys_addr.get() & !0x3_ffff_ffff) != virt_addr_raw {
                            continue;
                        }

                        // 1GB page
                        let virt_addr = (((pml4_i << 39) | (pml3_i << 30)) as u64
                            | (phys_addr.get() & 0x3_ffff_ffff))
                            .into();
                        return Ok(virt_addr);
                    }
                };
                for pml2_i in 0..PAGE_TABLE_ENTRY_LEN {
                    let pml2_entry = &pml2_table.entries[pml2_i];
                    if !pml2_entry.p() {
                        continue;
                    }

                    let pml1_table = match pml2_entry.page_table() {
                        Some(table) => table,
                        None => {
                            let virt_addr_raw = pml2_entry.addr();
                            if (phys_addr.get() & !0x1f_ffff) != virt_addr_raw {
                                continue;
                            }

                            // 2MB page
                            let virt_addr = (((pml4_i << 39) | (pml3_i << 30) | (pml2_i << 21))
                                as u64
                                | (phys_addr.get() & 0x1f_ffff))
                                .into();
                            return Ok(virt_addr);
                        }
                    };

                    for pml1_i in 0..PAGE_TABLE_ENTRY_LEN {
                        let pml1_entry = &pml1_table.entries[pml1_i];

                        if !pml1_entry.p() {
                            continue;
                        }

                        let virt_addr_raw = pml1_entry.addr();
                        if (phys_addr.get() & !0xfff) != virt_addr_raw {
                            continue;
                        }

                        let virt_addr =
                            (((pml4_i << 39) | (pml3_i << 30) | (pml2_i << 21) | (pml1_i << 12))
                                as u64
                                | (phys_addr.get() & 0xfff))
                                .into();
                        return Ok(virt_addr);
                    }
                }
            }
        }

        Err(PageManagerError::PhysicalAddressNotMappedError(phys_addr).into())
    }

    // unsupported 2MB / 1GB pages
    pub unsafe fn calc_phys_addr(&self, virt_addr: VirtualAddress) -> Result<PhysicalAddress> {
        let pml4e_index = virt_addr.get_pml4_entry_index();
        let pml3e_index = virt_addr.get_pml3_entry_index();
        let pml2e_index = virt_addr.get_pml2_entry_index();
        let pml1e_index = virt_addr.get_pml1_entry_index();
        let page_offset = virt_addr.get_page_offset();

        let pml4_table = self.pml4_table();
        let entry = &pml4_table.entries[pml4e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml3_table = entry.page_table().unwrap();
        let entry = &pml3_table.entries[pml3e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml2_table = match entry.page_table() {
            Some(table) => table,
            None => {
                unimplemented!();
            }
        };
        let entry = &pml2_table.entries[pml2e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml1_table = match entry.page_table() {
            Some(table) => table,
            None => {
                unimplemented!();
            }
        };
        let entry = &pml1_table.entries[pml1e_index];
        Ok(PhysicalAddress::new(entry.addr() | page_offset as u64))
    }

    pub unsafe fn create_new_page_table(
        &self,
        start: VirtualAddress,
        end: VirtualAddress,
        phys_addr: PhysicalAddress,
        rw: ReadWrite,
        mode: EntryMode,
        write_through_level: PageWriteThroughLevel,
    ) -> Result<()> {
        let pml4_table_mem_frame_info = bitmap::alloc_mem_frame(1)?;
        self.mem_clear(&pml4_table_mem_frame_info)?;
        let pml4_virt_addr =
            self.calc_virt_addr(pml4_table_mem_frame_info.frame_start_phys_addr)?;
        let pml4_page_table = &mut *pml4_virt_addr.as_ptr_mut::<PageTable>();

        let total_mem_size = bitmap::get_total_mem_size()?;
        for i in (start.get() as usize..total_mem_size.min(end.get() as usize)).step_by(PAGE_SIZE) {
            self.set_map(
                (i as u64).into(),
                phys_addr.offset(i - phys_addr.get() as usize),
                pml4_page_table,
                rw,
                mode,
                write_through_level,
                false,
            )?;
        }

        let mut cr3 = self.cr3();
        cr3.set_raw(pml4_virt_addr.get());
        cr3.write();

        Ok(())
    }

    pub unsafe fn update_mapping(&self, new_mapping_info: &MappingInfo) -> Result<()> {
        let pml4_virt_addr = VirtualAddress::new(self.cr3().raw());
        let pml4_page_table = &mut *pml4_virt_addr.as_ptr_mut::<PageTable>();

        let MappingInfo {
            start,
            end,
            phys_addr,
            rw,
            pwt,
            us,
            pcd,
        } = *new_mapping_info;

        for i in (start.get() as usize..end.get() as usize).step_by(PAGE_SIZE) {
            let virt_addr = (i as u64).into();

            self.set_map(
                virt_addr,
                phys_addr.offset(i - start.get() as usize),
                pml4_page_table,
                rw,
                us,
                pwt,
                pcd,
            )?;
        }

        Ok(())
    }

    pub unsafe fn page_table_entry(&self, virt_addr: VirtualAddress) -> Result<&PageTableEntry> {
        if virt_addr.get() % PAGE_SIZE as u64 != 0 {
            return Err(
                PageManagerError::VirtualAddressNotAlignedByPageSizeError(virt_addr).into(),
            );
        }

        self.calc_phys_addr(virt_addr)?;

        let pml4e_index = virt_addr.get_pml4_entry_index();
        let pml3e_index = virt_addr.get_pml3_entry_index();
        let pml2e_index = virt_addr.get_pml2_entry_index();
        let pml1e_index = virt_addr.get_pml1_entry_index();

        let pml4_table = self.pml4_table();
        let entry = &pml4_table.entries[pml4e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml3_table = entry.page_table().unwrap();
        let entry = &pml3_table.entries[pml3e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml2_table = entry.page_table().unwrap();
        let entry = &pml2_table.entries[pml2e_index];

        if !entry.p() {
            return Err(PageManagerError::VirtualAddressNotMappedError(virt_addr).into());
        }

        let pml1_table = entry.page_table().unwrap();
        let entry = &pml1_table.entries[pml1e_index];
        Ok(entry)
    }

    fn cr3(&self) -> Cr3 {
        Cr3::read()
    }

    unsafe fn pml4_table(&self) -> &mut PageTable {
        let ptr = self.cr3().raw() as *mut PageTable;
        &mut *ptr
    }

    // unsupported 2MB / 1GB pages
    unsafe fn set_map(
        &self,
        virt_addr: VirtualAddress,
        phys_addr: PhysicalAddress,
        pml4_table: &mut PageTable,
        rw: ReadWrite,
        mode: EntryMode,
        write_through_level: PageWriteThroughLevel,
        disable_cache: bool,
    ) -> Result<()> {
        if virt_addr.get() == 0 {
            return Err(PageManagerError::VirtualAddressNotAllowedToMapError(virt_addr).into());
        }

        if virt_addr.get() % PAGE_SIZE as u64 != 0 {
            return Err(
                PageManagerError::VirtualAddressNotAlignedByPageSizeError(virt_addr).into(),
            );
        }

        let pml4e_index = virt_addr.get_pml4_entry_index();
        let pml3e_index = virt_addr.get_pml3_entry_index();
        let pml2e_index = virt_addr.get_pml2_entry_index();
        let pml1e_index = virt_addr.get_pml1_entry_index();

        let entry = &mut pml4_table.entries[pml4e_index];

        if !entry.p() {
            let mem_info = bitmap::alloc_mem_frame(1)?;
            self.mem_clear(&mem_info)?;
            let virt_addr = self.calc_virt_addr(mem_info.frame_start_phys_addr)?;
            entry.set_entry(
                virt_addr.get(),
                rw,
                mode,
                write_through_level,
                disable_cache,
            );
        }

        if entry.rw() < rw {
            entry.set_rw(rw);
        }

        if entry.us() < mode {
            entry.set_us(mode);
        }

        let pml3_table = entry.page_table().unwrap();
        let entry = &mut pml3_table.entries[pml3e_index];

        if !entry.p() {
            let mem_info = bitmap::alloc_mem_frame(1)?;
            self.mem_clear(&mem_info)?;
            let virt_addr = self.calc_virt_addr(mem_info.frame_start_phys_addr)?;
            entry.set_entry(
                virt_addr.get(),
                rw,
                mode,
                write_through_level,
                disable_cache,
            );
        }

        if entry.rw() < rw {
            entry.set_rw(rw);
        }

        if entry.us() < mode {
            entry.set_us(mode);
        }

        let pml2_table = entry.page_table().unwrap();
        let entry = &mut pml2_table.entries[pml2e_index];

        if !entry.p() {
            let mem_info = bitmap::alloc_mem_frame(1)?;
            self.mem_clear(&mem_info)?;
            let virt_addr = self.calc_virt_addr(mem_info.frame_start_phys_addr)?;
            entry.set_entry(
                virt_addr.get(),
                rw,
                mode,
                write_through_level,
                disable_cache,
            );
        }

        if entry.rw() < rw {
            entry.set_rw(rw);
        }

        if entry.us() < mode {
            entry.set_us(mode);
        }

        let pml1_table = entry.page_table().unwrap();
        let entry = &mut pml1_table.entries[pml1e_index];
        entry.set_entry(
            phys_addr.get(),
            rw,
            mode,
            write_through_level,
            disable_cache,
        );

        Ok(())
    }

    // use this function instead of mem::mem_clear()
    unsafe fn mem_clear(&self, mem_frame_info: &MemoryFrameInfo) -> Result<()> {
        let frame_size = mem_frame_info.frame_size;
        let start_virt_addr = self.calc_virt_addr(mem_frame_info.frame_start_phys_addr)?;

        // TODO: replace to other methods
        for offset in (0..frame_size).step_by(8) {
            let ref_value = start_virt_addr.offset(offset).as_ptr_mut() as *mut u64;
            *ref_value = 0;
        }

        Ok(())
    }
}

pub fn calc_virt_addr(phys_addr: PhysicalAddress) -> Result<VirtualAddress> {
    unsafe { PAGE_MAN.calc_virt_addr(phys_addr) }
}

pub fn calc_phys_addr(virt_addr: VirtualAddress) -> Result<PhysicalAddress> {
    unsafe { PAGE_MAN.calc_phys_addr(virt_addr) }
}

pub fn create_new_page_table(
    start: VirtualAddress,
    end: VirtualAddress,
    phys_addr: PhysicalAddress,
    rw: ReadWrite,
    mode: EntryMode,
    write_through_level: PageWriteThroughLevel,
) -> Result<()> {
    unsafe { PAGE_MAN.create_new_page_table(start, end, phys_addr, rw, mode, write_through_level)? }
    info!(
        "paging: Created new page table (virt 0x{:x}-0x{:x} -> phys 0x{:x}-0x{:x})",
        start.get(),
        end.get(),
        phys_addr.get(),
        phys_addr.offset((end.get() - start.get()) as usize).get()
    );
    Ok(())
}

pub fn update_mapping(mapping_info: &MappingInfo) -> Result<()> {
    unsafe { PAGE_MAN.update_mapping(mapping_info) }
    // info!(
    //     "paging: Updated mapping (virt 0x{:x}-0x{:x} -> phys 0x{:x}-0x{:x})",
    //     start.get(),
    //     end.get(),
    //     phys_addr.get(),
    //     phys_addr.offset((end.get() - start.get()) as usize).get()
    // );
}

pub fn read_page_table_entry(virt_addr: VirtualAddress) -> Result<PageTableEntry> {
    Ok(unsafe { PAGE_MAN.page_table_entry(virt_addr)?.clone() })
}

#[test_case]
fn test_map_identity() {
    // already mapped by identity
    assert_eq!(calc_phys_addr(0xabcd000.into()).unwrap().get(), 0xabcd000);
    assert_eq!(calc_virt_addr(0xabcd123.into()).unwrap().get(), 0xabcd123);
    assert_eq!(calc_virt_addr(0xdeadbeaf.into()).unwrap().get(), 0xdeadbeaf);
}

#[test_case]
fn test_page_table_entry() {
    let virt_addr = VirtualAddress::new(0x3000000);
    let phys_addr = PhysicalAddress::new(0x4000000);
    let size = PAGE_SIZE * 10;

    assert!(update_mapping(&MappingInfo {
        start: virt_addr,
        end: virt_addr.offset(size),
        phys_addr,
        rw: ReadWrite::Read,
        us: EntryMode::User,
        pwt: PageWriteThroughLevel::WriteThrough,
        pcd: false,
    })
    .is_ok());

    let entry = read_page_table_entry(virt_addr).unwrap();

    assert!(entry.p());
    assert_eq!(entry.rw(), ReadWrite::Read);
    assert_eq!(entry.us(), EntryMode::User);
    assert_eq!(entry.pwt(), PageWriteThroughLevel::WriteThrough);
    assert_eq!(entry.addr(), phys_addr.get());

    assert!(update_mapping(&MappingInfo {
        start: virt_addr,
        end: virt_addr.offset(size),
        phys_addr: virt_addr.get().into(),
        rw: ReadWrite::Write,
        us: EntryMode::Supervisor,
        pwt: PageWriteThroughLevel::WriteThrough,
        pcd: false,
    })
    .is_ok());

    let entry = read_page_table_entry(virt_addr).unwrap();

    assert!(entry.p());
    assert_eq!(entry.rw(), ReadWrite::Write);
    assert_eq!(entry.us(), EntryMode::Supervisor);
    assert_eq!(entry.pwt(), PageWriteThroughLevel::WriteThrough);
    assert_eq!(entry.addr(), virt_addr.get());
}

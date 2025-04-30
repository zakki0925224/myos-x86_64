use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::mem::size_of;

const MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Bit32,
    Bit64,
    Other(u8),
}

impl From<u8> for Class {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::Bit32,
            2 => Self::Bit64,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Data {
    LittleEndian,
    BigEndian,
    Other(u8),
}

impl From<u8> for Data {
    fn from(value: u8) -> Self {
        match value {
            1 => Self::LittleEndian,
            2 => Self::BigEndian,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Relocatable,
    Executable,
    Shared,
    Core,
    Other(u16),
}

impl From<u16> for Type {
    fn from(value: u16) -> Self {
        match value {
            1 => Self::Relocatable,
            2 => Self::Executable,
            3 => Self::Shared,
            4 => Self::Core,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Machine {
    None,
    Sparc,
    X86,
    Mips,
    PowerPc,
    Arm,
    SuperH,
    Ia64,
    X8664,
    Aarch64,
    RiscV,
    Other(u16),
}

impl From<u16> for Machine {
    fn from(value: u16) -> Self {
        match value {
            0x00 => Self::None,
            0x02 => Self::Sparc,
            0x03 => Self::X86,
            0x08 => Self::Mips,
            0x14 => Self::PowerPc,
            0x28 => Self::Arm,
            0x2a => Self::SuperH,
            0x32 => Self::Ia64,
            0x3e => Self::X8664,
            0xb7 => Self::Aarch64,
            0xf3 => Self::RiscV,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Elf64Header {
    pub magic: [u8; 4],
    class: u8,
    data: u8,
    pub version0: u8,
    pub os_abi: u8,
    pub abi_version: u8,
    reserved: [u8; 7],
    type_: u16,
    machine: u16,
    pub verison1: u8,
    pub entry_point: u64,
    pub ph_offset: u64,
    pub sh_offset: u64,
    pub flags: u32,
    pub eh_size: u16,
    pub ph_entry_size: u16,
    pub ph_num: u16,
    pub sh_entry_size: u16,
    pub sh_num: u16,
    pub sh_str_index: u16,
}

impl Elf64Header {
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
    }

    pub fn class(&self) -> Class {
        self.class.into()
    }

    pub fn data(&self) -> Data {
        self.data.into()
    }

    pub fn elf_type(&self) -> Type {
        self.type_.into()
    }

    pub fn machine(&self) -> Machine {
        self.machine.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentType {
    Null,
    Load,
    Dynamic,
    Interpreter,
    Note,
    Reserved,
    ProgramHeader,
    ThreadLocalStorage,
    OsSpecLow,
    OsSpecHigh,
    ProcSpecLow,
    ProcSpecHigh,
    Other(u32),
}

impl From<u32> for SegmentType {
    fn from(value: u32) -> Self {
        match value {
            0x00000000 => Self::Null,
            0x00000001 => Self::Load,
            0x00000002 => Self::Dynamic,
            0x00000003 => Self::Interpreter,
            0x00000004 => Self::Note,
            0x00000005 => Self::Reserved,
            0x00000006 => Self::ProgramHeader,
            0x00000007 => Self::ThreadLocalStorage,
            0x60000000 => Self::OsSpecLow,
            0x6fffffff => Self::OsSpecHigh,
            0x70000000 => Self::ProcSpecLow,
            0x7fffffff => Self::ProcSpecHigh,
            x => SegmentType::Other(x),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentFlags {
    Executable,
    Writable,
    Readable,
    Other(u32),
}

impl From<u32> for SegmentFlags {
    fn from(value: u32) -> Self {
        match value {
            0x1 => Self::Executable,
            0x2 => Self::Writable,
            0x4 => Self::Readable,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Elf64ProgramHeader {
    segment_type: u32,
    flags: u32,
    pub offset: u64,
    pub virt_addr: u64,
    pub phys_addr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}

impl Elf64ProgramHeader {
    pub fn segment_type(&self) -> SegmentType {
        self.segment_type.into()
    }

    pub fn flags(&self) -> SegmentFlags {
        self.flags.into()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionHeaderType {
    Null,
    Program,
    SymbolTable,
    StringTable,
    RelocationEntriesWithAddends,
    SymbolHashTable,
    DynamicLinkInfo,
    Notes,
    Bss,
    RelocationEntries,
    Reserved,
    DynamicLinkSymbolTable,
    Constructors,
    Destructors,
    PreConstructors,
    Group,
    SymbolTableSectionHeaderIndex,
    NumberOfDefinedTypes,
    OsSpec,
    Other(u32),
}

impl From<u32> for SectionHeaderType {
    fn from(value: u32) -> Self {
        match value {
            0x00 => Self::Null,
            0x01 => Self::Program,
            0x02 => Self::SymbolTable,
            0x03 => Self::StringTable,
            0x04 => Self::RelocationEntriesWithAddends,
            0x05 => Self::SymbolHashTable,
            0x06 => Self::DynamicLinkInfo,
            0x07 => Self::Notes,
            0x08 => Self::Bss,
            0x09 => Self::RelocationEntries,
            0x0a => Self::Reserved,
            0x0b => Self::DynamicLinkSymbolTable,
            0x0e => Self::Constructors,
            0x0f => Self::Destructors,
            0x10 => Self::PreConstructors,
            0x11 => Self::Group,
            0x12 => Self::SymbolTableSectionHeaderIndex,
            0x13 => Self::NumberOfDefinedTypes,
            0x60000000 => Self::OsSpec,
            x => Self::Other(x),
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Elf64SectionHeader {
    pub name: u32,
    header_type: u32,
    pub flags: u64,
    pub addr: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub addr_align: u64,
    pub entry_size: u64,
}

impl Elf64SectionHeader {
    pub fn header_type(&self) -> SectionHeaderType {
        self.header_type.into()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Elf64Error {
    InvalidMagicNumberError,
}

#[derive(Debug)]
pub struct Elf64<'a> {
    data: &'a [u8],
}

impl<'a> Elf64<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Elf64Error> {
        let header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

        if !header.is_valid() {
            return Err(Elf64Error::InvalidMagicNumberError);
        }

        Ok(Self { data })
    }

    pub fn header(&self) -> &Elf64Header {
        unsafe { &*(self.data.as_ptr() as *const Elf64Header) }
    }

    pub fn program_headers(&self) -> Vec<&Elf64ProgramHeader> {
        let header = self.header();
        let ph_len = header.ph_num;
        let ph_offset = header.ph_offset;

        let mut phs = Vec::new();
        for i in 0..ph_len {
            let ph = unsafe {
                &*(self.data.as_ptr().offset(
                    ph_offset as isize + size_of::<Elf64ProgramHeader>() as isize * i as isize,
                ) as *const Elf64ProgramHeader)
            };
            phs.push(ph);
        }

        phs
    }

    pub fn section_headers(&self) -> Vec<&Elf64SectionHeader> {
        let header = self.header();
        let sh_len = header.sh_num;
        let sh_offset = header.sh_offset;

        let mut shs = Vec::new();
        for i in 0..sh_len {
            let sh = unsafe {
                &*(self.data.as_ptr().offset(
                    sh_offset as isize + size_of::<Elf64SectionHeader>() as isize * i as isize,
                ) as *const Elf64SectionHeader)
            };
            shs.push(sh);
        }

        shs
    }

    pub fn section_header_by_name(&self, name: &str) -> Option<&Elf64SectionHeader> {
        let section_headers = self.section_headers();
        section_headers
            .iter()
            .find(|sh| self.get_section_name_from_string_table(sh) == name)
            .copied()
    }

    pub fn get_section_name_from_string_table(
        &self,
        section_header: &Elf64SectionHeader,
    ) -> String {
        let no_name = "<NO NAME>".to_string();

        let name_offset = section_header.name as usize;
        let string_table = match self.string_table() {
            Some(strtab) => strtab,
            None => {
                return no_name;
            }
        };

        if string_table.len() < name_offset {
            return no_name;
        }

        let name_vec: Vec<u8> = string_table[name_offset..]
            .iter()
            .cloned()
            .take_while(|c| *c != 0)
            .collect();

        String::from_utf8_lossy(&name_vec).to_string()
    }

    pub fn data_by_section_header(&self, section_header: &Elf64SectionHeader) -> Option<&[u8]> {
        let offset = section_header.offset as usize;
        let size = section_header.size as usize;

        if offset == 0 || size == 0 {
            return None;
        }

        if self.data.len() < offset + size {
            return None;
        }

        Some(&self.data[offset..offset + size])
    }

    pub fn data_by_program_header(&self, program_header: &Elf64ProgramHeader) -> Option<&[u8]> {
        let offset = program_header.offset as usize;
        let file_size = program_header.file_size as usize;
        let mem_size = program_header.mem_size as usize;

        if program_header.segment_type() != SegmentType::Load {
            return None;
        }

        if file_size == 0 || mem_size == 0 {
            return None;
        }

        if self.data.len() < offset + file_size {
            return None;
        }

        Some(&self.data[offset..offset + file_size])
    }

    fn string_table(&self) -> Option<&[u8]> {
        let strtab_section_header = self
            .section_headers()
            .into_iter()
            .find(|h| h.header_type() == SectionHeaderType::StringTable)?;

        self.data_by_section_header(strtab_section_header)
    }
}

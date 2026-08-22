use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Attribute {
    ReadOnly = 0x01,
    Hidden = 0x02,
    System = 0x04,
    VolumeLabel = 0x08,
    LongFileName = 0x0f,
    Directory = 0x10,
    Archive = 0x20,
    Device = 0x40,
}

#[derive(Debug, PartialEq, Eq)]
pub enum EntryType {
    Null,
    Unused,
    LongFileName,
    Data,
}

pub trait ShortFileNameEntry {
    fn sf_name(&self) -> Option<String>;
}

pub trait LongFileNameEntry {
    fn lfn_entry_index(&self) -> Option<usize>;
    fn lf_name(&self) -> Option<String>;
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct DirectoryEntry([u8; 32]);

impl DirectoryEntry {
    pub fn raw(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn attr(&self) -> Option<Attribute> {
        match self.raw()[11] {
            0x01 => Some(Attribute::ReadOnly),
            0x02 => Some(Attribute::Hidden),
            0x04 => Some(Attribute::System),
            0x08 => Some(Attribute::VolumeLabel),
            0x0f => Some(Attribute::LongFileName),
            0x10 => Some(Attribute::Directory),
            0x20 => Some(Attribute::Archive),
            0x40 => Some(Attribute::Device),
            _ => None,
        }
    }

    pub fn entry_type(&self) -> EntryType {
        match self.raw()[0] {
            0x00 => return EntryType::Null,
            0xe5 => return EntryType::Unused,
            _ => (),
        }

        match self.raw()[10] {
            0x0f => EntryType::LongFileName,
            _ => EntryType::Data,
        }
    }

    pub fn first_cluster_num(&self) -> usize {
        let raw = self.raw();

        ((u16::from_le_bytes([raw[20], raw[21]]) as u32) << 16
            | u16::from_le_bytes([raw[26], raw[27]]) as u32) as usize
    }

    // bytes
    pub fn file_size(&self) -> usize {
        let raw = self.raw();
        u32::from_le_bytes([raw[28], raw[29], raw[30], raw[31]]) as usize
    }

    fn is_lf_name_entry(&self) -> bool {
        matches!(self.attr(), Some(Attribute::LongFileName))
    }
}

impl ShortFileNameEntry for DirectoryEntry {
    fn sf_name(&self) -> Option<String> {
        match self.attr()? {
            Attribute::Directory | Attribute::Archive | Attribute::VolumeLabel => (),
            _ => return None,
        }

        Some(String::from_utf8_lossy(&self.raw()[0..11]).into_owned())
    }
}

impl LongFileNameEntry for DirectoryEntry {
    fn lfn_entry_index(&self) -> Option<usize> {
        if !self.is_lf_name_entry() {
            return None;
        }

        Some(self.raw()[0] as usize)
    }

    fn lf_name(&self) -> Option<String> {
        if !self.is_lf_name_entry() {
            return None;
        }

        let mut utf16_buf = Vec::new();
        let raw_s = self.raw();

        for i in (1..11).step_by(2) {
            if utf16_buf.iter().find(|&&f| f == 0x0).is_some() {
                continue;
            }

            utf16_buf.push(raw_s[i] as u16 | raw_s[i + 1] as u16);
        }

        for i in (14..26).step_by(2) {
            if utf16_buf.iter().find(|&&f| f == 0x0).is_some() {
                continue;
            }

            utf16_buf.push(raw_s[i] as u16 | raw_s[i + 1] as u16);
        }

        for i in (28..32).step_by(2) {
            if utf16_buf.iter().find(|&&f| f == 0x0).is_some() {
                continue;
            }

            utf16_buf.push(raw_s[i] as u16 | raw_s[i + 1] as u16);
        }

        Some(String::from_utf16_lossy(&utf16_buf).replace("\0", ""))
    }
}

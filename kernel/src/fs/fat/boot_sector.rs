use super::volume::FatType;
use alloc::string::String;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BootSectorFat32OtherField {
    fat_size32: [u8; 4],
    ext_flags: [u8; 2],
    fs_version: [u8; 2],
    root_cluster: [u8; 4],
    fs_info: [u8; 2],
    backup_boot_sector: [u8; 2],
    reserved0: [u8; 12],
    drive_num: u8,
    reserved1: u8,
    ext_boot_sign: u8,
    volume_id: [u8; 4],
    volume_label: [u8; 11],
    fs_type: [u8; 8],
    boot_code32: [u8; 420],
    boot_sign: [u8; 2],
}

impl BootSectorFat32OtherField {
    pub fn fs_info_sector_num(&self) -> usize {
        u16::from_le_bytes(self.fs_info) as usize
    }

    pub fn fat_size(&self) -> usize {
        u32::from_le_bytes(self.fat_size32) as usize
    }

    pub fn root_cluster_num(&self) -> usize {
        u32::from_le_bytes(self.root_cluster) as usize
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct BootSector {
    jmp_boot: [u8; 3],
    oem_name: [u8; 8],
    bytes_per_sector: [u8; 2],
    sectors_per_cluster: u8,
    reserved_sector_count: [u8; 2],
    num_fats: u8,
    root_entry_count: [u8; 2],
    total_sector16: [u8; 2],
    media: u8,
    fat_size16: [u8; 2],
    sectors_per_track: [u8; 2],
    num_heads: [u8; 2],
    hidden_sectors: [u8; 4],
    total_sector32: [u8; 4],
    //pub other_field: BootSectorOtherField,
    other_field: [u8; 476],
}

impl BootSector {
    pub fn fat_type(&self) -> FatType {
        match self.data_clusters() {
            ..=4085 => FatType::Fat12,
            4086..=65525 => FatType::Fat16,
            _ => FatType::Fat32,
        }
    }

    pub fn oem_name(&self) -> String {
        String::from_utf8_lossy(&self.oem_name).into_owned()
    }

    pub fn fat32_other_field(&self) -> Option<BootSectorFat32OtherField> {
        if self.fat_type() != FatType::Fat32 {
            return None;
        }

        Some(unsafe {
            core::mem::transmute::<[u8; 476], BootSectorFat32OtherField>(self.other_field)
        })
    }

    pub fn data_clusters(&self) -> usize {
        self.data_sectors16() / self.sectors_per_cluster as usize
    }

    pub fn bytes_per_sector(&self) -> usize {
        u16::from_le_bytes(self.bytes_per_sector) as usize
    }

    pub fn sectors_per_cluster(&self) -> usize {
        self.sectors_per_cluster as usize
    }

    pub fn num_fats(&self) -> usize {
        self.num_fats as usize
    }

    pub fn fat_sectors16(&self) -> usize {
        self.fat_size16() * self.num_fats()
    }

    pub fn fat_sectors32(&self) -> Option<usize> {
        self.fat32_other_field()
            .map(|other_field| other_field.fat_size() * self.num_fats())
    }

    pub fn total_sectors(&self) -> usize {
        let total_sector16 = u16::from_le_bytes(self.total_sector16);
        let total_sector32 = u32::from_le_bytes(self.total_sector32);

        match total_sector16 {
            0 => total_sector32 as usize,
            _ => total_sector16 as usize,
        }
    }

    // fat start sector
    pub fn reserved_sectors(&self) -> usize {
        u16::from_le_bytes(self.reserved_sector_count) as usize
    }

    fn fat_size16(&self) -> usize {
        u16::from_le_bytes(self.fat_size16) as usize
    }

    fn root_entry_count(&self) -> usize {
        u16::from_le_bytes(self.root_entry_count) as usize
    }

    pub fn root_dir_start_sector16(&self) -> usize {
        self.reserved_sectors() + self.fat_sectors16()
    }

    pub fn root_dir_sectors16(&self) -> usize {
        (self.root_entry_count() * 32 + self.bytes_per_sector() - 1) / self.bytes_per_sector()
    }

    pub fn data_start_sector16(&self) -> usize {
        self.root_dir_start_sector16() + self.root_dir_sectors16()
    }

    pub fn data_sectors16(&self) -> usize {
        self.total_sectors() - self.data_start_sector16()
    }

    pub fn data_start_sector32(&self) -> Option<usize> {
        self.fat_sectors32()
            .map(|fat_sectors| self.reserved_sectors() + fat_sectors)
    }

    pub fn data_sectors32(&self) -> Option<usize> {
        self.data_start_sector32()
            .map(|data_start_sector| self.total_sectors() - data_start_sector)
    }
}

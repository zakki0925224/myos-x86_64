use crate::{
    arch::VirtualAddress,
    fs::fat::{
        boot_sector::BootSector, dir_entry::DirectoryEntry, file_allocation_table::ClusterType,
        fs_info_sector::FsInfoSector,
    },
};
use alloc::vec::Vec;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FatType {
    Fat12,
    Fat16,
    Fat32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FatVolume {
    volume_start_virt_addr: VirtualAddress,
}

impl FatVolume {
    pub fn new(volume_start_virt_addr: VirtualAddress) -> Self {
        Self {
            volume_start_virt_addr,
        }
    }

    pub fn boot_sector(&self) -> &BootSector {
        unsafe { &*(self.volume_start_virt_addr.as_ptr() as *const BootSector) }
    }

    pub fn fs_info_sector(&self) -> Option<&FsInfoSector> {
        match self.fat_type() {
            FatType::Fat32 => {
                let boot_sector = self.boot_sector();
                let fat32_other_field = boot_sector.fat32_other_field().unwrap();
                let fs_info_sector = unsafe {
                    &*(self
                        .volume_start_virt_addr
                        .offset(
                            fat32_other_field.fs_info_sector_num() * boot_sector.bytes_per_sector(),
                        )
                        .as_ptr() as *const FsInfoSector)
                };

                Some(fs_info_sector)
            }
            _ => None,
        }
    }

    pub fn fat_type(&self) -> FatType {
        let boot_sector = self.boot_sector();
        boot_sector.fat_type()
    }

    pub fn root_cluster_num(&self) -> usize {
        match self.fat_type() {
            FatType::Fat12 => unimplemented!(),
            FatType::Fat16 => unimplemented!(),
            FatType::Fat32 => (),
        }

        let boot_sector = self.boot_sector();
        let fat32_other_field = boot_sector.fat32_other_field().unwrap();
        fat32_other_field.root_cluster_num()
    }

    pub fn read_chained_dir_entries(&self, start_cluster_num: usize) -> Vec<DirectoryEntry> {
        let mut entries = Vec::new();
        let mut current_cluster_num = start_cluster_num;
        let mut next_cluster_num = self.next_cluster_num(current_cluster_num);

        loop {
            entries.extend(self.dir_entries(current_cluster_num));

            match next_cluster_num {
                Some(cluster_type) => match &cluster_type {
                    ClusterType::Data(next_cluster_num) => current_cluster_num = *next_cluster_num,
                    _ => break,
                },
                None => break,
            }
            next_cluster_num = self.next_cluster_num(current_cluster_num);
        }

        entries
    }

    fn dir_entries(&self, cluster_num: usize) -> Vec<&DirectoryEntry> {
        let boot_sector = self.boot_sector();
        let mut entries = Vec::with_capacity(self.dir_entries_per_cluster());

        if cluster_num < 2 || cluster_num >= self.clusters_cnt() {
            return entries;
        }

        match self.fat_type() {
            FatType::Fat12 => unimplemented!(),
            FatType::Fat16 => unimplemented!(),
            FatType::Fat32 => (),
        }

        for i in 0..entries.capacity() {
            let offset = boot_sector.data_start_sector32().unwrap()
                * boot_sector.bytes_per_sector()
                + boot_sector.bytes_per_sector()
                    * boot_sector.sectors_per_cluster()
                    * (cluster_num - 2)
                + size_of::<DirectoryEntry>() * i;
            let entry = unsafe {
                &*(self.volume_start_virt_addr.offset(offset).as_ptr() as *const DirectoryEntry)
            };
            entries.push(entry);
        }

        entries
    }

    // read file allocation table
    fn next_cluster_num(&self, cluster_num: usize) -> Option<ClusterType> {
        let boot_sector = self.boot_sector();
        match self.fat_type() {
            FatType::Fat12 => unimplemented!(),
            FatType::Fat16 => unimplemented!(),
            FatType::Fat32 => (),
        }

        let offset = boot_sector.reserved_sectors() * boot_sector.bytes_per_sector()
            + size_of::<u32>() * cluster_num;
        let ref_value =
            unsafe { &*(self.volume_start_virt_addr.offset(offset).as_ptr() as *const _) };
        let value = u32::from_le_bytes(*ref_value) as usize;

        match value {
            0xffffff8.. => Some(ClusterType::EndOfChain),
            0xffffff7.. => Some(ClusterType::Bad(value)),
            0xffffff0.. => Some(ClusterType::Reserved),
            0x2.. => Some(ClusterType::Data(value)),
            0x1 => Some(ClusterType::Reserved),
            0x0 => Some(ClusterType::Free),
        }
    }

    fn max_dir_entry_num(&self) -> usize {
        let boot_sector = self.boot_sector();
        let data_sectors = match self.fat_type() {
            FatType::Fat12 | FatType::Fat16 => boot_sector.data_sectors16(),
            FatType::Fat32 => boot_sector.data_sectors32().unwrap(),
        };

        data_sectors * boot_sector.bytes_per_sector() / size_of::<DirectoryEntry>()
    }

    fn dir_entries_per_cluster(&self) -> usize {
        let boot_sector = self.boot_sector();
        let cluster_size_bytes = boot_sector.bytes_per_sector() * boot_sector.sectors_per_cluster();
        cluster_size_bytes / size_of::<DirectoryEntry>()
    }

    fn clusters_cnt(&self) -> usize {
        let boot_sector = self.boot_sector();
        boot_sector.data_clusters()
    }
}

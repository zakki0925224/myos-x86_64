use super::path::Path;
use crate::error::{Error, Result};
use alloc::{collections::vec_deque::VecDeque, string::String, vec::Vec};
use dir_entry::*;
use volume::FatVolume;

pub mod boot_sector;
pub mod dir_entry;
pub mod file_allocation_table;
pub mod fs_info_sector;
pub mod volume;

#[derive(Debug, Clone)]
pub struct FileMetaData {
    pub name: String,
    pub attr: Attribute,
    pub size: usize,
    pub target_cluster_num: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Fat {
    volume: FatVolume,
    root_cluster_num: usize,
}

impl Fat {
    pub fn new(volume: FatVolume) -> Self {
        let root_cluster_num = volume.root_cluster_num();

        Self {
            volume,
            root_cluster_num,
        }
    }

    pub fn cluster_num(
        &self,
        dir_name: &str,
        current_dir_cluster_num: Option<usize>,
    ) -> Result<usize> {
        if current_dir_cluster_num.is_none()
            || current_dir_cluster_num == Some(self.root_cluster_num)
        {
            match dir_name {
                "." | ".." => return Ok(self.root_cluster_num),
                _ => (),
            }
        }

        let files = self.scan_dir(current_dir_cluster_num);
        let dir = files
            .iter()
            .find(|f| f.attr == Attribute::Directory && f.name.trim() == dir_name)
            .ok_or::<Error>("The directory does not exist".into())?;

        Ok(dir.target_cluster_num)
    }

    pub fn get_file(
        &self,
        file_name: &str,
        current_dir_cluster_num: Option<usize>,
    ) -> Result<(FileMetaData, Vec<u8>)> {
        let files = self.scan_dir(current_dir_cluster_num);
        let file = files
            .iter()
            .find(|f| f.attr == Attribute::Archive && f.name.trim() == file_name)
            .ok_or::<Error>("The file does not exist".into())?;

        let dir_entries = self
            .volume
            .read_chained_dir_entries(file.target_cluster_num);
        let mut bytes: Vec<u8> = dir_entries.iter().flat_map(|de| *de.raw()).collect();
        bytes.resize(file.size, 0);

        Ok((file.clone(), bytes))
    }

    pub fn get_file_by_abs_path(&self, path: &Path) -> Result<(FileMetaData, Vec<u8>)> {
        let mut current_dir_cluster_num = self.root_cluster_num;
        let path = path.normalize();
        let parent_path = path.parent();

        for dir_name in parent_path.names() {
            current_dir_cluster_num = self.cluster_num(dir_name, Some(current_dir_cluster_num))?;
        }

        let file = self.get_file(&path.name(), Some(current_dir_cluster_num))?;
        Ok(file)
    }

    pub fn scan_dir(&self, dir_cluster_num: Option<usize>) -> Vec<FileMetaData> {
        let dir_cluster_num = match dir_cluster_num {
            Some(cluster_num) => cluster_num,
            None => self.root_cluster_num,
        };
        let mut files = Vec::new();

        let mut lf_name_buf = VecDeque::new();
        let dir_entries = self.volume.read_chained_dir_entries(dir_cluster_num);

        for i in 0..dir_entries.len() {
            let dir_entry = dir_entries[i];
            let entry_type = dir_entry.entry_type();
            let file_attr = dir_entry.attr();

            // end of not null directories
            if entry_type == EntryType::Null && file_attr.is_none() {
                break;
            }

            // long file name entry
            if let (Some(lf_name), Some(lfn_entry_index)) =
                (dir_entry.lf_name(), dir_entry.lfn_entry_index())
            {
                if lfn_entry_index >= 1 {
                    lf_name_buf.push_front(lf_name);
                    continue;
                }
            }

            match file_attr {
                Some(attr) => match attr {
                    Attribute::Archive | Attribute::Directory => {
                        let file_name = if lf_name_buf.len() > 0 {
                            lf_name_buf.iter().fold(String::new(), |acc, s| acc + s)
                        } else {
                            dir_entry.sf_name().unwrap()
                        };

                        let file = FileMetaData {
                            name: file_name,
                            attr,
                            size: dir_entry.file_size(),
                            target_cluster_num: dir_entry.first_cluster_num(),
                        };

                        files.push(file);
                        lf_name_buf.clear();
                    }
                    _ => (),
                },
                None => (),
            }
        }

        files
    }
}

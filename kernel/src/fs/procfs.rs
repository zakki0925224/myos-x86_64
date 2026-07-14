use crate::{
    error::Result,
    fs::{
        path::Path,
        vfs::{FileSystem, FsFileType, FsMetaData, VirtualFileSystemError},
    },
    util::time,
};
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use core::cmp::min;

enum ProcNode {
    Root,
    Uptime,
}

impl ProcNode {
    fn read(&self) -> Result<Vec<u8>> {
        match self {
            Self::Root => Err(VirtualFileSystemError::NotFile(None).into()),
            Self::Uptime => {
                let uptime = time::global_uptime();
                let ms = uptime.as_millis();
                let bytes = format!("{}.{:02}\n", ms / 1000, (ms % 1000) / 10);
                Ok(bytes.as_bytes().to_vec())
            }
        }
    }

    fn write(&self, _data: &[u8]) -> Result<()> {
        Err(VirtualFileSystemError::ReadOnly(None).into())
    }

    fn metadata(&self) -> FsMetaData {
        match self {
            Self::Root => FsMetaData {
                file_type: FsFileType::Directory,
                size: 0,
            },
            Self::Uptime => FsMetaData {
                file_type: FsFileType::File,
                size: 0,
            },
        }
    }
}

pub struct ProcFs;

impl FileSystem for ProcFs {
    fn read_entry_names(&self, path: &Path) -> Result<Vec<String>> {
        match self.path_to_node(&path.normalize())? {
            ProcNode::Root => Ok(vec!["uptime".to_string()]),
            _ => Err(VirtualFileSystemError::NotDirectory(Some(path.clone())).into()),
        }
    }

    fn read_file(&self, path: &Path, offset: usize, max_len: usize) -> Result<Vec<u8>> {
        let normalized_path = path.normalize();
        let node = self.path_to_node(&normalized_path)?;
        let bytes = node.read()?;

        let start = min(offset, bytes.len());
        let end = min(start.saturating_add(max_len), bytes.len());

        Ok(bytes[start..end].to_vec())
    }

    fn write_file(&self, path: &Path, _offset: usize, data: &[u8]) -> Result<()> {
        let normalized_path = path.normalize();
        let node = self.path_to_node(&normalized_path)?;
        node.write(data)
    }

    fn metadata(&self, path: &Path) -> Result<FsMetaData> {
        let normalized_path = path.normalize();
        let node = self.path_to_node(&normalized_path)?;
        Ok(node.metadata())
    }
}

impl ProcFs {
    fn path_to_node(&self, normalized_path: &Path) -> Result<ProcNode> {
        match normalized_path.names().as_slice() {
            [] => Ok(ProcNode::Root),
            ["uptime"] => Ok(ProcNode::Uptime),
            _ => Err(
                VirtualFileSystemError::NoSuchFileOrDirectory(Some(normalized_path.clone())).into(),
            ),
        }
    }
}

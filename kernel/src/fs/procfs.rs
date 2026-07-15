use crate::{
    error::Result,
    fs::{
        path::Path,
        vfs::{FileSystem, FsFileType, FsMetaData, VirtualFileSystemError},
    },
    task::{scheduler, TaskId},
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
    TaskDir(TaskId),
    TaskStatus(TaskId),
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
            Self::TaskDir(_) => Err(VirtualFileSystemError::NotFile(None).into()),
            Self::TaskStatus(task_id) => {
                let s = scheduler::task_snapshot(*task_id)
                    .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(None))?;
                let bytes = format!(
                    "Name:\t{}\nPid:\t{}\nPPid:\t{}\nState:\t{}\n",
                    s.name,
                    s.id,
                    s.parent.map_or("-".to_string(), |p| p.to_string()),
                    s.state,
                );
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
            Self::TaskDir(_) => FsMetaData {
                file_type: FsFileType::Directory,
                size: 0,
            },
            Self::TaskStatus(_) => FsMetaData {
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
            ProcNode::Root => {
                let mut names = vec!["uptime".to_string(), "self".to_string()];

                let mut task_ids = scheduler::task_ids();
                task_ids.sort_unstable();
                names.extend(task_ids.iter().map(|id| id.to_string()));

                Ok(names)
            }
            ProcNode::TaskDir(_) => Ok(vec!["status".to_string()]),
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
            [pid] => Ok(ProcNode::TaskDir(resolve_task_id(pid, normalized_path)?)),
            [pid, "status"] => Ok(ProcNode::TaskStatus(resolve_task_id(pid, normalized_path)?)),
            _ => Err(
                VirtualFileSystemError::NoSuchFileOrDirectory(Some(normalized_path.clone())).into(),
            ),
        }
    }
}

fn resolve_task_id(s: &str, path: &Path) -> Result<TaskId> {
    if s == "self" {
        return scheduler::current_task_id()
            .ok_or(VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())).into());
    }

    let id: TaskId = s
        .parse::<usize>()
        .map_err(|_| VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())))?
        .into();

    if scheduler::task_snapshot(id).is_none() {
        return Err(VirtualFileSystemError::NoSuchFileOrDirectory(Some(path.clone())).into());
    }

    Ok(id)
}
